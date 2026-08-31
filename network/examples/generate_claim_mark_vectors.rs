use serde_json::{json, Value};
use tohseno_network::claim_mark::{ClaimMark, ClaimMarkError, ClaimPoint};

fn main() {
    let clockwise = vec![
        point(80.0, 50.0),
        point(71.0, 29.0),
        point(50.0, 20.0),
        point(29.0, 29.0),
        point(20.0, 50.0),
        point(29.0, 71.0),
        point(50.0, 80.0),
        point(71.0, 71.0),
        point(80.0, 50.0),
    ];
    let counterclockwise = clockwise.iter().copied().rev().collect::<Vec<_>>();
    let vectors = vec![
        drawn("ordinary-clockwise", clockwise),
        drawn("ordinary-counterclockwise", counterclockwise),
        drawn(
            "irregular-expressive",
            vec![
                point(80.0, 50.0),
                point(72.0, 23.0),
                point(43.0, 18.0),
                point(18.0, 38.0),
                point(24.0, 73.0),
                point(55.0, 86.0),
                point(83.0, 68.0),
                point(80.0, 50.0),
            ],
        ),
        drawn(
            "wide-loop",
            vec![
                point(5.0, 50.0),
                point(10.0, 10.0),
                point(90.0, 10.0),
                point(95.0, 50.0),
                point(90.0, 90.0),
                point(10.0, 90.0),
                point(5.0, 50.0),
            ],
        ),
        drawn(
            "narrow-loop",
            vec![
                point(62.0, 50.0),
                point(58.0, 40.0),
                point(50.0, 38.0),
                point(40.0, 42.0),
                point(38.0, 50.0),
                point(42.0, 60.0),
                point(50.0, 62.0),
                point(60.0, 58.0),
                point(62.0, 50.0),
            ],
        ),
        drawn(
            "failed-line",
            vec![
                point(10.0, 10.0),
                point(30.0, 30.0),
                point(60.0, 60.0),
                point(90.0, 90.0),
            ],
        ),
        drawn("failed-tap", vec![point(50.0, 50.0)]),
        drawn(
            "failed-non-enclosing",
            vec![
                point(8.0, 8.0),
                point(38.0, 8.0),
                point(38.0, 38.0),
                point(8.0, 38.0),
                point(8.0, 8.0),
            ],
        ),
        mark_value(
            "accessibility-hold",
            "accessibility_hold",
            None,
            ClaimMark::accessibility_hold(),
        ),
    ];
    let fixture = json!({
        "schema": "tohseno.claim-mark-vectors/1",
        "encoding": {
            "domain_utf8": "TOHSENO-CLAIM-MARK-V1\0",
            "kind": { "drawn": 0, "accessibility_hold": 1 },
            "point_count": 64,
            "coordinate": "u16be",
            "quantization": "floor(clamp(value,0,1)*65535+0.5)"
        },
        "vectors": vectors
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&fixture).expect("serialize")
    );
}

fn point(x: f64, y: f64) -> ClaimPoint {
    ClaimPoint { x, y }
}

fn drawn(id: &str, points: Vec<ClaimPoint>) -> Value {
    match ClaimMark::from_canvas_stroke(&points, 100.0, 100.0) {
        Ok(mark) => mark_value(id, "drawn", Some(points), mark),
        Err(error) => json!({
            "id": id,
            "kind": "drawn",
            "canvas": { "width": 100.0, "height": 100.0 },
            "points": points,
            "accepted": false,
            "error": error_name(&error),
            "canonical_hex": null,
            "gesture_commitment": null
        }),
    }
}

fn mark_value(id: &str, kind: &str, points: Option<Vec<ClaimPoint>>, mark: ClaimMark) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "canvas": points.as_ref().map(|_| json!({ "width": 100.0, "height": 100.0 })),
        "points": points,
        "accepted": true,
        "error": null,
        "canonical_hex": hex(&mark.canonical_bytes()),
        "gesture_commitment": hex(&mark.gesture_commitment())
    })
}

fn error_name(error: &ClaimMarkError) -> &'static str {
    match error {
        ClaimMarkError::InvalidCanvas => "invalid_canvas",
        ClaimMarkError::InvalidPoint => "invalid_point",
        ClaimMarkError::TooShort => "too_short",
        ClaimMarkError::OpenStroke => "open_stroke",
        ClaimMarkError::DoesNotEncloseArtifact => "does_not_enclose_artifact",
        ClaimMarkError::InvalidEncoding => "invalid_encoding",
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(2 + bytes.len() * 2);
    encoded.push_str("0x");
    for byte in bytes {
        use std::fmt::Write;
        write!(encoded, "{byte:02x}").expect("hex");
    }
    encoded
}
