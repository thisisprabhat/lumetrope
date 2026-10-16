use lumetrope_core::geom::{self, Affine, Contour, Path};

fn line(len: f64) -> Path {
    Path { contours: vec![Contour { v: vec![[0.0, 0.0], [len, 0.0]], i: vec![[0.0; 2]; 2], o: vec![[0.0; 2]; 2], closed: false }] }
}

#[test]
fn rect_has_4_vertices_clockwise() {
    let c = geom::rect([0.0, 0.0], [20.0, 10.0], 0.0, false);
    assert_eq!(c.v, vec![[10.0, -5.0], [10.0, 5.0], [-10.0, 5.0], [-10.0, -5.0]]);
    assert!(c.closed);
}

#[test]
fn rect_roundness_adds_tangents() {
    let c = geom::rect([0.0, 0.0], [20.0, 20.0], 4.0, false);
    assert_eq!(c.v.len(), 8);
    assert!(c.o.iter().chain(c.i.iter()).any(|t| t[0] != 0.0 || t[1] != 0.0));
}

#[test]
fn ellipse_length_close_to_circumference() {
    let c = geom::ellipse([0.0, 0.0], [100.0, 100.0], false);
    let l = Path { contours: vec![c] }.length();
    assert!((l - std::f64::consts::PI * 100.0).abs() < 0.5, "len {l}");
}

#[test]
fn trim_half_of_line() {
    let out = geom::trim(&[line(100.0)], 0.0, 50.0, 0.0, false);
    let l: f64 = out.iter().map(|p| p.length()).sum();
    assert!((l - 50.0).abs() < 1e-3, "len {l}");
}

#[test]
fn trim_offset_wraps() {
    // closed square 0..10 perimeter 40, take 75%..125% (offset 270deg on 0..50%)
    let sq = Path { contours: vec![geom::rect([5.0, 5.0], [10.0, 10.0], 0.0, false)] };
    let out = geom::trim(&[sq], 0.0, 50.0, 270.0, false);
    let l: f64 = out.iter().map(|p| p.length()).sum();
    assert!((l - 20.0).abs() < 1e-3, "len {l}");
}

#[test]
fn trim_equal_start_end_is_empty() {
    let out = geom::trim(&[line(10.0)], 30.0, 30.0, 0.0, false);
    assert!(out.iter().all(|p| p.length() < 1e-9));
}

#[test]
fn star_5_points_has_10_vertices() {
    let c = geom::star(1, [0.0, 0.0], 5.0, 50.0, 20.0, 0.0, 0.0, 0.0, false);
    assert_eq!(c.v.len(), 10);
    assert!((c.v[0][1] + 50.0).abs() < 1e-9, "first point on top: {:?}", c.v[0]);
    let poly = geom::star(2, [0.0, 0.0], 6.0, 50.0, 0.0, 0.0, 0.0, 0.0, false);
    assert_eq!(poly.v.len(), 6);
}

#[test]
fn affine_lottie_order() {
    let m = Affine::from_lottie([10.0, 10.0], [100.0, 100.0], [200.0, 200.0], 0.0, 0.0, 0.0);
    assert_eq!(m.apply([10.0, 10.0]), [100.0, 100.0]);
    assert_eq!(m.apply([11.0, 10.0]), [102.0, 100.0]);
    let r = Affine::from_lottie([0.0, 0.0], [0.0, 0.0], [100.0, 100.0], 90.0, 0.0, 0.0);
    let p = r.apply([1.0, 0.0]);
    assert!(p[0].abs() < 1e-9 && (p[1] - 1.0).abs() < 1e-9, "rotation is clockwise on screen: {p:?}");
}

#[test]
fn round_corners_rounds_square() {
    let sq = geom::rect([0.0, 0.0], [20.0, 20.0], 0.0, false);
    let r = geom::round_corners(&sq, 5.0);
    assert_eq!(r.v.len(), 8);
}

#[test]
fn to_skia_builds_path_and_skips_empty() {
    assert!(Path { contours: vec![geom::rect([0.0, 0.0], [10.0, 10.0], 0.0, false)] }.to_skia().is_some());
    assert!(Path::default().to_skia().is_none());
}
