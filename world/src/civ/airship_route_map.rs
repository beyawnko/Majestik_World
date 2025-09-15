use crate::{
    CONFIG, Index, IndexRef,
    civ::airship_travel::{
        AirshipDockPlatform, AirshipRouteLeg, AirshipSpawningLocation, Airships, DockNode,
    },
    sim::{WorldSim, get_horizon_map, sample_pos, sample_wpos},
    util::{DHashMap, DHashSet},
};
use common::{
    assets::{self, AssetExt, BoxedError, FileAsset},
    terrain::{
        map::{MapConfig, MapSample, MapSizeLg},
        uniform_idx_as_vec2,
    },
};
use delaunator::{Point, Triangulation};
use serde::Deserialize;
use tiny_skia::{
    FillRule, FilterQuality, IntRect, IntSize, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

use std::{borrow::Cow, env, error::Error, path::PathBuf};
use tracing::error;
use vek::*;

// TODO(#1234): Airship route map planning
// - Integrate prevailing-wind model into cost function (favor tailwinds,
//   penalize headwinds)
// - Support no-fly zones and altitude bands in pathfinding constraints
// - Index no-fly polygons with an R-tree (e.g., rstar) to accelerate spatial
//   queries and reduce per-step pathfinding cost
// - Cache computed routes per (origin, destination, conditions) and invalidate
//   on world updates
// - Persist minimal route summaries to save files; rebuild detailed paths on
//   load
// Keep these notes in sync with the linked issue for status and design
// decisions.

/// Wrapper for Pixmap so that the FileAsset blanket `Asset` impl can be used.
/// This is necessary because Pixmap is in the tiny-skia crate.
pub struct PackedSpritesPixmap(pub Pixmap);

// Load PackedSpritesPixmap directly from PNG bytes via the FileAsset blanket
// Asset impl.
impl FileAsset for PackedSpritesPixmap {
    const EXTENSIONS: &'static [&'static str] = &["png"];

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, BoxedError> {
        Pixmap::decode_png(bytes.as_ref())
            .map(PackedSpritesPixmap)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e).into())
    }
}

/// Extension trait for tiny_skia::Pixmap.
/// The copy_region function is used in the TinySkiaSpriteMap to
/// cut out sprites from a larger image.
trait PixmapExt {
    fn bounds(&self) -> IntRect;
    fn copy_region(&self, rect: IntRect) -> Option<Pixmap>;
    fn draw_text<F>(
        &mut self,
        text: &str,
        center: Vec2<f32>,
        scale: f32,
        rotation: f32,
        sprite_map: &TinySkiaSpriteMap,
        id_formatter: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: Fn(char) -> String;
}

impl PixmapExt for Pixmap {
    /// Returns the Pixmap bounds or a 1x1 rectangle if the width or height is
    /// invalid (which should not happen due to the defensive design of
    /// IntRect and Pixmap).
    fn bounds(&self) -> IntRect {
        if let Some(bounds) = IntRect::from_xywh(0, 0, self.width(), self.height()) {
            bounds
        } else {
            IntRect::from_xywh(0, 0, 1, 1).unwrap()
        }
    }

    /// Createa a new Pixmap from a rectangle of the original Pixmap.
    fn copy_region(&self, rect: IntRect) -> Option<Pixmap> {
        if self.bounds().contains(&rect)
            && let Some(region_size) = IntSize::from_wh(rect.width(), rect.height())
            && let Some(from_rect) = self.bounds().intersect(&rect)
        {
            let stride = self.width() as i32 * tiny_skia::BYTES_PER_PIXEL as i32;
            let mut region_data = Vec::with_capacity(
                (from_rect.width() * from_rect.height()) as usize * tiny_skia::BYTES_PER_PIXEL,
            );

            for y in from_rect.top()..from_rect.bottom() {
                let row_start = y * stride + from_rect.left() * tiny_skia::BYTES_PER_PIXEL as i32;
                let row_end =
                    row_start + from_rect.width() as i32 * tiny_skia::BYTES_PER_PIXEL as i32;
                region_data.extend_from_slice(&self.data()[row_start as usize..row_end as usize]);
            }
            Pixmap::from_vec(region_data, region_size)
        } else {
            None
        }
    }

    /// Draws a text string on a tiny_skia::Pixmap.
    /// This draws text by drawing individual characters as tiny_skia::Pixmaps
    /// from a TinySkiaSpriteMap, given the center position of the string
    /// on the target Pixmap, the scale factor, and the rotation angle.
    fn draw_text<F>(
        &mut self,
        text: &str,
        center: Vec2<f32>,
        scale: f32,
        rotation: f32,
        sprite_map: &TinySkiaSpriteMap,
        id_formatter: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: Fn(char) -> String,
    {
        let char_count = text.len();
        if char_count == 0 {
            return Err("Text cannot be empty".into());
        }
        // Map the characters of the string to sprite IDs.
        let sprite_ids = text.chars().map(id_formatter).collect::<Vec<_>>();

        let sprites = sprite_map.get_sprites(sprite_ids);
        if sprites.len() != char_count {
            return Err(format!(
                "Sprite map contained only {} sprites for text '{}'",
                sprites.len(),
                text
            )
            .into());
        }

        let char_size = sprite_map.get_first_sprite_size();
        let text_width = char_count as f32 * char_size.width();
        let inverse_scale_factor = 1.0 / scale;
        let text_tlx = center.x - text_width / 2.0 * scale;
        let text_tly = (center.y - char_size.height() / 2.0 * scale) * inverse_scale_factor;

        let mut transform = if rotation.is_normal() {
            let rot_deg = rotation.to_degrees();
            Transform::from_rotate_at(rot_deg, center.x, center.y)
        } else {
            Transform::identity()
        };

        if scale.is_normal() && (scale - 1.0).abs() > f32::EPSILON {
            transform = transform.pre_scale(scale, scale);
        }

        let paint = PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        };

        for (char_index, sprite) in sprites.iter().enumerate() {
            // X is offset per char by the scaled char width.
            let x =
                (text_tlx + char_index as f32 * char_size.width() * scale) * inverse_scale_factor;
            self.draw_pixmap(
                // x and y are pre-scaled up because the rotation transform scales down the entire
                // coordinate system to scale down the text image but we don't want
                // to scale the text position.
                x as i32,
                text_tly as i32,
                sprite.as_ref(),
                &paint,
                transform,
                None,
            );
        }
        Ok(())
    }
}

/// Defines the location and size of a sprite in a packed sprite map image.
#[derive(Deserialize, Debug, Clone)]
struct TinySkiaSpriteMeta {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// The metadata for all sprites that were packed into a single image (texture).
#[derive(Deserialize, Debug, Clone)]
struct TinySkiaSpriteMapMeta {
    texture_width: u32,
    texture_height: u32,
    sprites_meta: Vec<TinySkiaSpriteMeta>,
}

/// Allows a TinySkiaSpriteMapMeta to be loaded using the asset system.
impl FileAsset for TinySkiaSpriteMapMeta {
    const EXTENSION: &'static str = "ron";

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, BoxedError> { assets::load_ron(&bytes) }
}

/// A set of sprites that are unpacked from a larger sprite map image.
pub struct TinySkiaSpriteMap {
    sprites: Vec<tiny_skia::Pixmap>,
    sprite_ids: DHashMap<String, usize>,
}

impl TinySkiaSpriteMap {
    /// Loads the sprite map metadata and the packed sprite image
    /// then unpacks the sprites and inserts them into a hash map
    /// with the sprite ID from the metadata as the key.
    fn new(img_spec: &str, meta_spec: &str) -> Self {
        let mut sprites = Vec::default();
        let mut sprite_ids = DHashMap::default();
        let map_meta = TinySkiaSpriteMapMeta::load(meta_spec);
        match map_meta {
            Ok(meta) => {
                let sprite_map_meta = meta.read();
                let packed_sprites_result = PackedSpritesPixmap::load(img_spec);
                match packed_sprites_result {
                    Ok(packed_sprites_handle) => {
                        let packed_sprites = &packed_sprites_handle.read().0;
                        for sprite_meta in sprite_map_meta.sprites_meta.iter() {
                            if let Some(sprite_frame) = IntRect::from_xywh(
                                sprite_meta.x,
                                sprite_meta.y,
                                sprite_meta.width,
                                sprite_meta.height,
                            ) && let Some(sprite) = packed_sprites.copy_region(sprite_frame)
                            {
                                // sprite.set_transform(Transform::from_scale(1.0, -1.0));
                                sprites.push(sprite);
                                sprite_ids.insert(sprite_meta.id.clone(), sprites.len() - 1);
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to load packed sprites: {:?}", e);
                    },
                }
            },
            Err(e) => {
                eprintln!("Failed to load meta: {:?}", e);
            },
        }
        TinySkiaSpriteMap {
            sprites,
            sprite_ids,
        }
    }

    /// Returns a reference to the sprite with the given ID.
    fn get_sprite(&self, id: &str) -> Option<&Pixmap> {
        if let Some(index) = self.sprite_ids.get(id) {
            return Some(&self.sprites[*index]);
        }
        None
    }

    fn get_sprites(&self, ids: Vec<String>) -> Vec<&Pixmap> {
        let mut sprites = Vec::new();
        for id in ids {
            if let Some(sprite) = self.get_sprite(&id) {
                sprites.push(sprite);
            }
        }
        sprites
    }

    fn get_first_sprite_size(&self) -> tiny_skia::Size {
        if let Some(sprite) = self.sprites.first() {
            return tiny_skia::Size::from_wh(sprite.width() as f32, sprite.height() as f32)
                .unwrap();
        }
        tiny_skia::Size::from_wh(0.0, 0.0).unwrap()
    }

    fn get_sprite_size(&self, id: &str) -> tiny_skia::Size {
        if let Some(index) = self.sprite_ids.get(id) {
            let sprite = &self.sprites[*index];
            return tiny_skia::Size::from_wh(sprite.width() as f32, sprite.height() as f32)
                .unwrap();
        }
        tiny_skia::Size::from_wh(0.0, 0.0).unwrap()
    }
}

/// Creates a basic world map as a tiny_skia::Pixmap
fn basic_world_pixmap(image_size: &MapSizeLg, index: &Index, sampler: &WorldSim) -> Option<Pixmap> {
    let horizons = get_horizon_map(
        *image_size,
        Aabr {
            min: Vec2::zero(),
            max: image_size.chunks().map(|e| e as i32),
        },
        CONFIG.sea_level,
        CONFIG.sea_level + sampler.max_height,
        |posi| {
            let sample = sampler.get(uniform_idx_as_vec2(*image_size, posi)).unwrap();

            sample.basement.max(sample.water_alt)
        },
        |a| a,
        |h| h,
    )
    .ok();

    let colors = index.colors();
    let features = index.features();
    let index_ref = IndexRef {
        colors: &colors,
        features: &features,
        index,
    };

    let mut map_config = MapConfig::orthographic(*image_size, 0.0..=sampler.max_height);
    map_config.horizons = horizons.as_ref();
    map_config.is_shaded = true;
    map_config.is_stylized_topo = true;
    let map = sampler.get_map(index_ref, None);

    if let Some(mut pixmap) =
        Pixmap::new(image_size.chunks().x as u32, image_size.chunks().y as u32)
    {
        let map_h = image_size.chunks().y as usize;
        let stride = pixmap.width() as usize * tiny_skia::BYTES_PER_PIXEL;
        let pixel_data = pixmap.data_mut();
        map_config.generate(
            |pos| {
                let default_sample = sample_pos(&map_config, sampler, index_ref, None, pos);
                let [r, g, b, _a] = map.rgba[pos].to_le_bytes();
                MapSample {
                    rgb: Rgb::new(r, g, b),
                    ..default_sample
                }
            },
            |wpos| sample_wpos(&map_config, sampler, wpos),
            |pos, (r, g, b, a)| {
                let pixel_index = (map_h - pos.y - 1) * stride + pos.x * tiny_skia::BYTES_PER_PIXEL;
                pixel_data[pixel_index] = r;
                pixel_data[pixel_index + 1] = g;
                pixel_data[pixel_index + 2] = b;
                pixel_data[pixel_index + 3] = a;
            },
        );
        Some(pixmap)
    } else {
        error!("Failed to create pixmap for world map");
        None
    }
}

/// Creates a tiny_skia::Pixmap of the basic triangulation over the docking
/// sites.
fn dock_sites_triangulation_map(
    triangulation: &Triangulation,
    points: &[Point],
    image_size: &MapSizeLg,
    index: Option<&Index>,
    sampler: Option<&WorldSim>,
    map_image_path: Option<&str>,
) -> Option<Pixmap> {
    let mut pixmap = if let Some(index) = index
        && let Some(sampler) = sampler
    {
        basic_world_pixmap(image_size, index, sampler)
    } else if let Some(map_image_path) = map_image_path {
        Pixmap::load_png(map_image_path)
            .map_err(|e| format!("Failed to load map image: {}", e))
            .ok()
    } else {
        None
    }?;
    let world_chunks = image_size.chunks();
    let world_blocks = world_chunks.map(|u| u as f32) * 32.0;
    let map_w = image_size.chunks().x as f32;
    let map_h = image_size.chunks().y as f32;

    // coordinates are in world blocks, convert to map pixels and invert y axis
    macro_rules! map_triangle_points {
        ($vec:expr) => {
            Vec2 {
                x: $vec.x as f32,
                y: $vec.y as f32,
            }
        };
    }

    macro_rules! flip_y {
        ($vec:expr) => {
            Vec2 {
                x: $vec.x,
                y: map_h - $vec.y,
            }
        };
    }

    // the triangles are triplets in a Vec<usize> so we need to iterate over them in
    // groups of 3. The macros are used to convert the points from world blocks
    // to map pixels and flip the y axis. map_triangles is a Vec of arrays of 3
    // Vec2s representing the 3 points of each triangle.
    let map_triangles = triangulation
        .triangles
        .chunks(3)
        .map(|triangle| {
            [
                flip_y!(map_triangle_points!(points[triangle[0]]) / world_blocks * map_w),
                flip_y!(map_triangle_points!(points[triangle[1]]) / world_blocks * map_w),
                flip_y!(map_triangle_points!(points[triangle[2]]) / world_blocks * map_w),
            ]
        })
        .collect::<Vec<_>>();

    let mut paint = Paint::default();
    paint.set_color_rgba8(105, 231, 255, 255);
    paint.anti_alias = true;

    let mut circled_points: DHashSet<Vec2<i32>> = DHashSet::default();
    let mut lines_drawn: DHashSet<(Vec2<i32>, Vec2<i32>)> = DHashSet::default();
    let mut circle_pb = PathBuilder::new();
    let mut lines_pb = PathBuilder::new();

    for triangle in map_triangles.iter() {
        // triangle is an array of 3 Vec2<f32> representing the 3 points of the
        // triangle.

        // Draw a circle around the points
        for p in triangle.iter() {
            let pi32 = Vec2::new(p.x as i32, p.y as i32);
            if !circled_points.contains(&pi32) {
                circle_pb.push_circle(p.x, p.y, 10.0);
                circled_points.insert(pi32);
            }
            // for (x, y) in BresenhamCircle::new(p.x as i32, p.y as i32, 10) {
            //     if x < 0 || y < 0 || x >= map_w as i32 || y >= map_h as i32 {
            //         continue;
            //     }
            //     image.put_pixel(x as u32, y as u32, [site_r, site_g, site_b,
            // 255].into()); }
        }

        // Now draw the triangle lines
        for i in 0..3 {
            let p1 = triangle[i];
            let p2 = triangle[(i + 1) % 3];
            let p1i32 = Vec2::new(p1.x as i32, p1.y as i32);
            let p2i32 = Vec2::new(p2.x as i32, p2.y as i32);
            if !lines_drawn.contains(&(p1i32, p2i32)) {
                // calculate where the triangle edge intersects a circle of radius 10 around
                // each point
                let dir = (p2 - p1).normalized();
                let start_edge_center = p1 + dir * 10.0;
                let end_edge_center = p2 - dir * 10.0;
                lines_pb.move_to(start_edge_center.x, start_edge_center.y);
                lines_pb.line_to(end_edge_center.x, end_edge_center.y);
                lines_drawn.insert((p1i32, p2i32));
            }

            // This is a simplified rectangle fill for the line to get more
            // thickness. fill_line(&mut image, &start_edge_center,
            // &end_edge_center, 3.0, [     route_r, route_g,
            // route_b, ]);
        }
    }

    let circle_stroke = Stroke {
        width: 2.0,
        ..Default::default()
    };
    match circle_pb.finish() {
        Some(path) => {
            pixmap.stroke_path(&path, &paint, &circle_stroke, Transform::identity(), None);
        },
        None => {
            eprintln!("Failed to draw circles path");
        },
    }

    let lines_stroke = Stroke {
        width: 3.0,
        ..Default::default()
    };
    match lines_pb.finish() {
        Some(path) => {
            pixmap.stroke_path(&path, &paint, &lines_stroke, Transform::identity(), None);
        },
        None => {
            eprintln!("Failed to draw lines path");
        },
    }

    Some(pixmap)
}

/// Creates a tiny_skia::Pixmap of the optimized docking sites tesselation
/// where the docking site nodes all have an even number of connections
/// to other docking sites.
fn dock_sites_optimized_tesselation_map(
    _triangulation: &Triangulation,
    points: &[Point],
    node_connections: &DHashMap<usize, DockNode>,
    image_size: MapSizeLg,
    index: &Index,
    sampler: &WorldSim,
) -> Option<Pixmap> {
    let mut pixmap = basic_world_pixmap(&image_size, index, sampler)?;

    let world_chunks = sampler.map_size_lg().chunks();
    let world_blocks = world_chunks.map(|u| u as f32) * 32.0;
    let map_w = image_size.chunks().x as f32;
    let map_h = image_size.chunks().y as f32;

    let map_points = points
        .iter()
        .map(|p| {
            Vec2::new(
                (p.x / world_blocks.x as f64 * map_w as f64) as f32,
                (map_h as f64 - (p.y / world_blocks.y as f64 * map_h as f64)) as f32,
            )
        })
        .collect::<Vec<_>>();

    let mut paint = Paint::default();
    paint.set_color_rgba8(105, 231, 255, 255);
    paint.anti_alias = true;

    let mut stroke = Stroke {
        width: 2.0,
        ..Default::default()
    };

    // Draw a circle around the points (the docking sites)
    let mut pb = PathBuilder::new();
    for dock_center in map_points.iter() {
        pb.push_circle(dock_center.x, dock_center.y, 10.0);
    }
    match pb.finish() {
        Some(path) => {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        },
        None => {
            eprintln!("Failed to create a circle path");
        },
    }

    // Draw the dock node connections
    pb = PathBuilder::new();

    stroke = Stroke {
        width: 3.0,
        ..Default::default()
    };

    let mut lines_drawn: DHashSet<(usize, usize)> = DHashSet::default();
    for (_, dock_node) in node_connections.iter() {
        if let Some(dp1) = map_points.get(dock_node.node_id) {
            dock_node.connected.iter().for_each(|cpid| {
                if let Some(dp2) = map_points.get(*cpid) {
                    if !lines_drawn.contains(&(dock_node.node_id, *cpid)) {
                        // calculate where the line intersects a circle of radius 10 around
                        // each point
                        let dir = (dp2 - dp1).normalized();
                        let ep1 = dp1 + dir * 10.0;
                        let ep2 = dp2 - dir * 10.0;
                        pb.move_to(ep1.x, ep1.y);
                        pb.line_to(ep2.x, ep2.y);
                        lines_drawn.insert((dock_node.node_id, *cpid));
                    }
                }
            });
        }
    }

    match pb.finish() {
        Some(path) => {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        },
        None => {
            eprintln!("Failed to create a lines path");
        },
    }

    Some(pixmap)
}

/// Draws the route segment loops (segments) on the provided tiny_skia::Pixmap,
/// where the segments are loops of docking points derived from the
/// eulerian circuit created from the graph of docking sites.
///
/// # Arguments
///
/// * `routes`: The route loops, where each inner vector contains the end point
///   of each route leg (the AirshipRouteLeg). The route loops, so the 'from'
///   point of the first leg is the last item of the inner vector. Docking
///   positions are on the cardinal sides of the docking sites.
///   AirshipDockPlatform::NorthPlatform means the airship will dock on the
///   north side of the dock.
/// * `points`: The docking site locations in pixmap coordinates (top left is
///   the origin).
/// * `pixmap`: The Pixmap on which to draw the segments.
///
/// This draws circles around the docking locations and lines for the route
/// legs. The coordinates must be pre-scaled to the pixmap size. The Veloren
/// world uses a bottom-left origin with coordinates in world blocks, so world
/// coordinates must be converted by inverting the y-axix and scaling to the
/// pixmap size.
fn draw_airship_routes(
    routes: &[Vec<AirshipRouteLeg>],
    points: &[Vec2<f32>],
    spawning_points: &[Vec<Vec2<f32>>],
    pixmap: &mut Pixmap,
) -> Result<(), Box<dyn Error>> {
    // Draw a circle around the points (the docking sites)
    let mut pb: PathBuilder = PathBuilder::new();
    for dock_center in points.iter() {
        pb.push_circle(dock_center.x, dock_center.y, 10.0);
    }

    let mut paint = Paint::default();
    paint.set_color_rgba8(105, 231, 255, 255);
    paint.anti_alias = true;

    let stroke = Stroke {
        width: 2.0,
        ..Default::default()
    };

    let path = pb
        .finish()
        .ok_or_else(|| "Failed to create path for circles".to_string())?;
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);

    // Red, Green, Blue, Yellow
    // Segment lines are drawn in these colors in the order they are found in the
    // segments vector (i.e. the outer segments vector).
    let segment_colors = [[255u8, 0u8, 0u8], [0u8, 255u8, 0u8], [6u8, 218u8, 253u8], [
        255u8, 255u8, 0u8,
    ]];

    let stroke = Stroke {
        width: 3.0,
        ..Default::default()
    };

    let loc_fn = |point: &Vec2<f32>, platform: &AirshipDockPlatform| -> (f32, f32) {
        match platform {
            AirshipDockPlatform::NorthPlatform => (point.x, point.y - 10.0),
            AirshipDockPlatform::SouthPlatform => (point.x, point.y + 10.0),
            AirshipDockPlatform::EastPlatform => (point.x + 10.0, point.y),
            AirshipDockPlatform::WestPlatform => (point.x - 10.0, point.y),
        }
    };

    // Draw the route segment lines
    for (i, route) in routes.iter().enumerate() {
        let color: [u8; 3] = segment_colors[i % segment_colors.len()];
        paint.set_color_rgba8(color[0], color[1], color[2], 255);

        if route.len() > 1 {
            let mut prev_leg = &route[route.len() - 1];
            let mut pb = PathBuilder::new();
            for route_leg in route.iter() {
                let from_loc = loc_fn(&points[prev_leg.dest_index], &prev_leg.platform);
                let to_loc = loc_fn(&points[route_leg.dest_index], &route_leg.platform);
                pb.move_to(from_loc.0, from_loc.1);
                pb.line_to(to_loc.0, to_loc.1);
                prev_leg = route_leg;
            }
            let path = pb
                .finish()
                .ok_or_else(|| "Failed to create path for lines".to_string())?;
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    // The map is hard to read without an indication of which direction the lines
    // are traversed and in which order, so we draw the route line numbers on
    // the map with the line numbers drawn at the destination end of the line to
    // orient the reader.
    let route_color_ids = ["RED", "GREEN", "BLUE", "YELLOW"];
    let digits_sprite_map = TinySkiaSpriteMap::new(
        "world.module.airship.airship_route_map_digits",
        "world.module.airship.airship_route_map_digits",
    );
    let digit_size = digits_sprite_map.get_sprite_size("RED_0");
    for (i, route) in routes.iter().enumerate() {
        let id_formatter =
            |c: char| format!("{}_{}", route_color_ids[i % route_color_ids.len()], c);

        if route.len() > 1 {
            let mut leg_line_number = 1;
            let mut prev_leg = &route[route.len() - 1];

            // The leg line numbers are drawn at the destination end of the line.
            for route_leg in route.iter() {
                // for j in 0..segment.len() - 1 {
                let from_loc = loc_fn(&points[prev_leg.dest_index], &prev_leg.platform);
                let to_loc = loc_fn(&points[route_leg.dest_index], &route_leg.platform);
                let p1 = Vec2::new(from_loc.0, from_loc.1);
                let p2 = Vec2::new(to_loc.0, to_loc.1);
                let dir = (p2 - p1).normalized();

                // Turn the number into a string with leading zeros for single digit numbers.
                let rln_str = format!("{:03}", leg_line_number);

                // Draw the digits so they are aligned with the direction the segment line
                // will be traversed. Y axis is inverted in the image.
                let angle = Airships::angle_between_vectors_cw(dir, -Vec2::unit_y());

                // Draw the digits 80% of the way along the segment line or one digit height
                // away from the circle at the end of the segment line, whichever is greater.
                let p1p2dist = p1.distance(p2) - 20.0; // subtract the radius of the circles
                let seg_num_offset = (p1p2dist * 0.20).max(digit_size.height());
                let seg_num_center = p2 - dir * seg_num_offset;
                // let seg_num_center = p2 - dir * (10.0 + seg_num_offset);

                pixmap.draw_text(
                    &rln_str,
                    seg_num_center,
                    0.75,
                    angle,
                    &digits_sprite_map,
                    id_formatter,
                )?;

                leg_line_number += 1;
                prev_leg = route_leg;
            }
        }
    }

    // Draw a filled circle for the airship spawning locations.
    spawning_points
        .iter()
        .enumerate()
        .for_each(|(route_index, points)| {
            let mut pb: PathBuilder = PathBuilder::new();
            for pt in points.iter() {
                pb.push_circle(pt.x, pt.y, 5.0);
            }

            let mut paint = Paint::default();
            let color: [u8; 3] = segment_colors[route_index % segment_colors.len()];
            paint.set_color_rgba8(color[0], color[1], color[2], 255);
            paint.anti_alias = true;

            match pb.finish() {
                Some(path) => {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                },
                None => {
                    eprintln!("Failed to create path for drawing spawning points");
                },
            }
        });

    Ok(())
}

/// Creates a tiny_skia::Pixmap of the airship route segments
/// where the segments are loops of docking points derived from the
/// eulerian circuit created from the eulerized tesselation.
fn airship_routes_map(
    routes: &[Vec<AirshipRouteLeg>],
    points: &[Point],
    spawning_locations: Option<&Vec<AirshipSpawningLocation>>,
    image_size: &MapSizeLg,
    index: Option<&Index>,
    sampler: Option<&WorldSim>,
    map_image_path: Option<&str>,
) -> Option<Pixmap> {
    let mut pixmap = if let Some(index) = index
        && let Some(sampler) = sampler
    {
        basic_world_pixmap(image_size, index, sampler)
    } else if let Some(map_image_path) = map_image_path {
        Pixmap::load_png(map_image_path)
            .map_err(|e| format!("Failed to load map image: {}", e))
            .ok()
    } else {
        None
    }?;

    let world_chunks = image_size.chunks();
    let world_blocks = world_chunks.map(|u| u as f32) * 32.0;
    let map_w = image_size.chunks().x as f32;
    let map_h = image_size.chunks().y as f32;

    let map_points = points
        .iter()
        .map(|p| {
            Vec2::new(
                (p.x / world_blocks.x as f64 * map_w as f64) as f32,
                (map_h as f64 - (p.y / world_blocks.y as f64 * map_h as f64)) as f32,
            )
        })
        .collect::<Vec<_>>();

    let mut spawning_points = Vec::new();
    if let Some(spawning_locations) = spawning_locations {
        for route_index in 0..4 {
            let mut route_spawning_locations = Vec::new();
            for spawning_location in spawning_locations.iter() {
                if spawning_location.route_index == route_index {
                    route_spawning_locations.push(Vec2::new(
                        spawning_location.pos.x / world_blocks.x * map_w,
                        map_h - (spawning_location.pos.y / world_blocks.y * map_h),
                    ));
                }
            }
            if route_spawning_locations.is_empty() {
                continue;
            }
            spawning_points.push(route_spawning_locations);
        }
    }

    if let Err(e) = draw_airship_routes(routes, &map_points, &spawning_points, &mut pixmap) {
        error!("Failed to draw airship route segments: {}", e);
        return None;
    }

    Some(pixmap)
}

pub fn save_airship_routes_triangulation(
    triangulation: &Triangulation,
    points: &[Point],
    image_size: &MapSizeLg,
    seed: u32,
    index: Option<&Index>,
    sampler: Option<&WorldSim>,
    map_image_path: Option<&str>,
) {
    let airship_routes_log_folder = env::var("AIRSHIP_ROUTES_LOG_FOLDER").ok();
    if let Some(routes_log_folder) = airship_routes_log_folder {
        let world_map_file = format!(
            "{}/airship_docks_triangulation_{}.png",
            routes_log_folder, seed
        );
        let world_map_file_path = PathBuf::from(world_map_file);
        if let Some(pixmap) = dock_sites_triangulation_map(
            triangulation,
            points,
            image_size,
            index,
            sampler,
            map_image_path,
        ) {
            if pixmap.save_png(&world_map_file_path).is_err() {
                error!("Failed to save airship routes triangulation map");
            }
        }
    }
}

pub fn save_airship_route_segments(
    routes: &[Vec<AirshipRouteLeg>],
    points: &[Point],
    spawning_locations: &Vec<AirshipSpawningLocation>,
    image_size: &MapSizeLg,
    seed: u32,
    index: Option<&Index>,
    sampler: Option<&WorldSim>,
    map_image_path: Option<&str>,
) {
    let airship_routes_log_folder = env::var("AIRSHIP_ROUTES_LOG_FOLDER").ok();
    if let Some(routes_log_folder) = airship_routes_log_folder {
        let routes_with_spawning_file = format!(
            "{}/airship_routes_with_spawn_locations_map_{}.png",
            routes_log_folder, seed
        );
        if let Some(pixmap) = airship_routes_map(
            routes,
            points,
            Some(spawning_locations),
            image_size,
            index,
            sampler,
            map_image_path,
        ) {
            if pixmap.save_png(&routes_with_spawning_file).is_err() {
                error!("Failed to save airship route segments with spawning locations map");
            }
        }
        let routes_only_file =
            format!("{}/airship_routes_only_map_{}.png", routes_log_folder, seed);
        if let Some(pixmap) = airship_routes_map(
            routes,
            points,
            None,
            image_size,
            index,
            sampler,
            map_image_path,
        ) {
            if pixmap.save_png(&routes_only_file).is_err() {
                error!("Failed to save airship route segments only map");
            }
        }
    }
}

pub fn export_world_map(index: &Index, sampler: &WorldSim) -> Result<(), String> {
    let airship_routes_log_folder = env::var("AIRSHIP_ROUTES_LOG_FOLDER").ok();
    let routes_log_folder = airship_routes_log_folder
        .ok_or("AIRSHIP_ROUTES_LOG_FOLDER environment variable is not set".to_string())?;
    let world_map_file = format!("{}/basic_world_map{}.png", routes_log_folder, index.seed);
    if let Some(world_map) = basic_world_pixmap(&sampler.map_size_lg(), index, sampler) {
        if world_map.save_png(&world_map_file).is_err() {
            error!("Failed to save world map");
        }
    }
    Ok(())
}

pub fn export_docknodes(
    map_image_path: &str,
    points: &[Point],
    node_connections: &DHashMap<usize, DockNode>,
    color: [u8; 3],
    output_path: &str,
) -> Result<(), String> {
    let mut pixmap =
        Pixmap::load_png(map_image_path).map_err(|e| format!("Failed to load map image: {}", e))?;

    let mut circle_pb: PathBuilder = PathBuilder::new();
    let mut line_pb: PathBuilder = PathBuilder::new();
    let mut lines_drawn: DHashSet<(usize, usize)> = DHashSet::default();

    node_connections.iter().for_each(|(_, dock_node)| {
        // Draw a circle around the dock center
        let dock_center = &points[dock_node.node_id];
        circle_pb.push_circle(dock_center.x as f32, dock_center.y as f32, 10.0);

        // Draw lines to connected nodes
        for connected_node_id in &dock_node.connected {
            if !lines_drawn.contains(&(dock_node.node_id, *connected_node_id)) {
                let connected_node = &points[*connected_node_id];
                line_pb.move_to(dock_center.x as f32, dock_center.y as f32);
                line_pb.line_to(connected_node.x as f32, connected_node.y as f32);
                lines_drawn.insert((dock_node.node_id, *connected_node_id));
            }
        }
    });

    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], 255);
    paint.anti_alias = true;

    let stroke = Stroke {
        width: 2.0,
        ..Default::default()
    };
    let path = circle_pb
        .finish()
        .ok_or_else(|| "Failed to create path for circles".to_string())?;
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);

    let stroke = Stroke {
        width: 3.0,
        ..Default::default()
    };
    let path = line_pb
        .finish()
        .ok_or_else(|| "Failed to create path for lines".to_string())?;
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);

    pixmap
        .save_png(output_path)
        .map_err(|e| format!("Failed to save output image: {}", e))
}
