use tinyglow::{ClassicGlow, Extent, Quality};

#[test]
fn classic_preserves_reference_constants_at_half_resolution() {
    let glow = ClassicGlow {
        color: [128, 255, 0],
        thickness: 120.0,
        intensity: 0.0,
        quality: Quality::Half,
    };
    let extent = Extent::new(81, 25).unwrap();
    let passes = glow.passes(extent);

    assert_eq!(passes.mask_extent, Extent::new(40, 12).unwrap());
    assert_eq!(passes.output_extent, extent);
    assert_eq!(passes.horizontal, [1.0 / 40.0, 0.0, 1.0, 0.0]);
    assert_eq!(passes.vertical, [0.0, 1.0 / 40.0, 100.0, 0.0]);
    assert!((passes.composite[0] - 0.219_519_73).abs() < 1e-6);
    assert_eq!(&passes.composite[1..], &[1.0, 0.0, 0.1]);
    assert_eq!(
        glow.mask_extent(Extent::new(1, 1).unwrap()),
        Extent::new(1, 1).unwrap()
    );
}
