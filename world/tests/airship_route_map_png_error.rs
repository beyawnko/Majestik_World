#![cfg(feature = "airship_maps")]

use std::{borrow::Cow, error::Error, io};

use common::assets::FileAsset;
use veloren_world::civ::airship_route_map::PackedSpritesPixmap;

#[test]
fn png_decode_error_includes_context() {
    let bogus = Cow::from(&b"not a png"[..]);
    let err = match PackedSpritesPixmap::from_bytes(bogus) {
        Ok(_) => panic!("expected decode failure"),
        Err(err) => err,
    };
    let io_err = err
        .downcast::<io::Error>()
        .expect("error should downcast to io::Error");
    assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    assert!(
        io_err.to_string().contains("Failed to decode PNG"),
        "error message missing PNG context: {}",
        io_err
    );
    assert!(
        io_err.source().is_some(),
        "decode error source should be preserved"
    );
}
