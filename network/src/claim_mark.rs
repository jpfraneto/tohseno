use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const DOMAIN: &[u8] = b"TOHSENO-CLAIM-MARK-V1\0";
const POINT_COUNT: usize = 64;
const MIN_ARC_LENGTH: f64 = 0.70;
const MIN_ENCLOSURE_SPAN: f64 = 0.24;
const MIN_CENTER_MARGIN: f64 = 0.075;
const MAX_ENDPOINT_DISTANCE: f64 = 0.22;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClaimMarkKind {
    Drawn = 0,
    AccessibilityHold = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimMark {
    kind: ClaimMarkKind,
    points: [(u16, u16); POINT_COUNT],
}

#[derive(Debug, Error, PartialEq)]
pub enum ClaimMarkError {
    #[error("the Claim canvas must have finite positive dimensions")]
    InvalidCanvas,
    #[error("the Claim stroke contains a point outside its finite canvas")]
    InvalidPoint,
    #[error("draw one continuous loop around the app")]
    TooShort,
    #[error("bring the end of the line back near where it began")]
    OpenStroke,
    #[error("draw the boundary around the app")]
    DoesNotEncloseArtifact,
    #[error("the canonical Claim mark encoding is invalid")]
    InvalidEncoding,
}

impl ClaimMark {
    pub fn from_canvas_stroke(
        stroke: &[ClaimPoint],
        canvas_width: f64,
        canvas_height: f64,
    ) -> Result<Self, ClaimMarkError> {
        if !canvas_width.is_finite()
            || !canvas_height.is_finite()
            || canvas_width <= 0.0
            || canvas_height <= 0.0
        {
            return Err(ClaimMarkError::InvalidCanvas);
        }
        if stroke.len() < 4 {
            return Err(ClaimMarkError::TooShort);
        }
        let mut normalized = Vec::with_capacity(stroke.len());
        for point in stroke {
            if !point.x.is_finite()
                || !point.y.is_finite()
                || point.x < 0.0
                || point.y < 0.0
                || point.x > canvas_width
                || point.y > canvas_height
            {
                return Err(ClaimMarkError::InvalidPoint);
            }
            let next = ClaimPoint {
                x: point.x / canvas_width,
                y: point.y / canvas_height,
            };
            if normalized
                .last()
                .is_none_or(|prior| distance(*prior, next) > f64::EPSILON)
            {
                normalized.push(next);
            }
        }
        if normalized.len() < 4 {
            return Err(ClaimMarkError::TooShort);
        }

        let total = arc_length(&normalized);
        if total < MIN_ARC_LENGTH {
            return Err(ClaimMarkError::TooShort);
        }
        if distance(normalized[0], *normalized.last().expect("nonempty")) > MAX_ENDPOINT_DISTANCE {
            return Err(ClaimMarkError::OpenStroke);
        }
        if !substantially_encloses_center(&normalized) {
            return Err(ClaimMarkError::DoesNotEncloseArtifact);
        }

        Ok(Self {
            kind: ClaimMarkKind::Drawn,
            points: resample_and_quantize(&normalized, total),
        })
    }

    pub fn accessibility_hold() -> Self {
        Self {
            kind: ClaimMarkKind::AccessibilityHold,
            points: ACCESSIBILITY_HOLD_POINTS,
        }
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ClaimMarkError> {
        let expected = DOMAIN.len() + 1 + 2 + POINT_COUNT * 4;
        if bytes.len() != expected || !bytes.starts_with(DOMAIN) {
            return Err(ClaimMarkError::InvalidEncoding);
        }
        let kind = match bytes[DOMAIN.len()] {
            0 => ClaimMarkKind::Drawn,
            1 => ClaimMarkKind::AccessibilityHold,
            _ => return Err(ClaimMarkError::InvalidEncoding),
        };
        if u16::from_be_bytes([bytes[DOMAIN.len() + 1], bytes[DOMAIN.len() + 2]]) as usize
            != POINT_COUNT
        {
            return Err(ClaimMarkError::InvalidEncoding);
        }
        let mut points = [(0, 0); POINT_COUNT];
        let mut offset = DOMAIN.len() + 3;
        for point in &mut points {
            point.0 = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
            point.1 = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
            offset += 4;
        }
        if kind == ClaimMarkKind::AccessibilityHold && points != ACCESSIBILITY_HOLD_POINTS {
            return Err(ClaimMarkError::InvalidEncoding);
        }
        Ok(Self { kind, points })
    }

    pub fn kind(&self) -> ClaimMarkKind {
        self.kind
    }

    pub fn quantized_points(&self) -> &[(u16, u16); POINT_COUNT] {
        &self.points
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DOMAIN.len() + 3 + POINT_COUNT * 4);
        bytes.extend_from_slice(DOMAIN);
        bytes.push(self.kind as u8);
        bytes.extend_from_slice(&(POINT_COUNT as u16).to_be_bytes());
        for (x, y) in self.points {
            bytes.extend_from_slice(&x.to_be_bytes());
            bytes.extend_from_slice(&y.to_be_bytes());
        }
        bytes
    }

    pub fn gesture_commitment(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_bytes()).into()
    }

    pub fn normalized_points(&self) -> [ClaimPoint; POINT_COUNT] {
        self.points.map(|(x, y)| ClaimPoint {
            x: f64::from(x) / f64::from(u16::MAX),
            y: f64::from(y) / f64::from(u16::MAX),
        })
    }
}

fn resample_and_quantize(points: &[ClaimPoint], total: f64) -> [(u16, u16); POINT_COUNT] {
    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0);
    for pair in points.windows(2) {
        cumulative
            .push(cumulative.last().copied().unwrap_or_default() + distance(pair[0], pair[1]));
    }

    let mut result = [(0, 0); POINT_COUNT];
    let mut segment = 0;
    for (index, target) in (0..POINT_COUNT)
        .map(|index| total * index as f64 / (POINT_COUNT - 1) as f64)
        .enumerate()
    {
        while segment + 1 < cumulative.len() - 1 && cumulative[segment + 1] < target {
            segment += 1;
        }
        let start = points[segment];
        let end = points[segment + 1];
        let span = cumulative[segment + 1] - cumulative[segment];
        let fraction = if span <= f64::EPSILON {
            0.0
        } else {
            (target - cumulative[segment]) / span
        };
        result[index] = (
            quantize(start.x + (end.x - start.x) * fraction),
            quantize(start.y + (end.y - start.y) * fraction),
        );
    }
    result
}

fn substantially_encloses_center(points: &[ClaimPoint]) -> bool {
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    if max_x - min_x < MIN_ENCLOSURE_SPAN
        || max_y - min_y < MIN_ENCLOSURE_SPAN
        || min_x > 0.5 - MIN_CENTER_MARGIN
        || max_x < 0.5 + MIN_CENTER_MARGIN
        || min_y > 0.5 - MIN_CENTER_MARGIN
        || max_y < 0.5 + MIN_CENTER_MARGIN
    {
        return false;
    }

    let center = ClaimPoint { x: 0.5, y: 0.5 };
    let mut inside = false;
    let mut prior = *points.last().expect("nonempty");
    for &point in points {
        let crosses = (point.y > center.y) != (prior.y > center.y)
            && center.x
                < (prior.x - point.x) * (center.y - point.y) / (prior.y - point.y) + point.x;
        if crosses {
            inside = !inside;
        }
        prior = point;
    }
    inside
}

fn arc_length(points: &[ClaimPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum()
}

fn distance(a: ClaimPoint, b: ClaimPoint) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn quantize(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * f64::from(u16::MAX) + 0.5).floor() as u16
}

// A canonical counter-clockwise ring used only by the accessibility hold path.
// These fixed integers avoid fabricating hand geometry or depending on
// platform-specific trigonometric rounding.
const ACCESSIBILITY_HOLD_POINTS: [(u16, u16); POINT_COUNT] = [
    (57343, 32768),
    (57222, 30359),
    (56871, 27974),
    (56287, 25636),
    (55476, 23368),
    (54444, 21192),
    (53199, 19130),
    (51754, 17201),
    (50122, 15426),
    (48320, 13823),
    (46364, 12411),
    (44272, 11203),
    (42067, 10211),
    (39769, 9444),
    (37399, 8908),
    (34981, 8610),
    (32543, 8549),
    (30110, 8728),
    (27706, 9144),
    (25354, 9793),
    (23072, 10670),
    (20882, 11767),
    (18806, 13073),
    (16864, 14576),
    (15077, 16261),
    (13462, 18111),
    (12035, 20108),
    (10812, 22232),
    (9812, 24462),
    (9042, 26776),
    (8508, 29151),
    (8213, 31568),
    (8153, 34005),
    (8333, 36438),
    (8749, 38842),
    (9398, 41194),
    (10275, 43476),
    (11372, 45666),
    (12678, 47742),
    (14181, 49684),
    (15866, 51471),
    (17716, 53086),
    (19713, 54513),
    (21837, 55736),
    (24067, 56736),
    (26381, 57506),
    (28756, 58040),
    (31173, 58335),
    (33610, 58395),
    (36043, 58215),
    (38447, 57799),
    (40799, 57150),
    (43081, 56273),
    (45271, 55176),
    (47347, 53870),
    (49289, 52367),
    (51076, 50682),
    (52691, 48832),
    (54118, 46835),
    (55341, 44711),
    (56341, 42481),
    (57111, 40167),
    (57645, 37792),
    (57343, 32768),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct VectorFixture {
        schema: String,
        encoding: serde_json::Value,
        vectors: Vec<Vector>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Vector {
        id: String,
        kind: String,
        canvas: Option<Canvas>,
        points: Option<Vec<ClaimPoint>>,
        accepted: bool,
        error: Option<String>,
        canonical_hex: Option<String>,
        gesture_commitment: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Canvas {
        width: f64,
        height: f64,
    }

    #[test]
    fn expressive_loop_is_accepted_without_circularity_scoring() {
        let stroke = [
            ClaimPoint { x: 80.0, y: 50.0 },
            ClaimPoint { x: 72.0, y: 23.0 },
            ClaimPoint { x: 43.0, y: 18.0 },
            ClaimPoint { x: 18.0, y: 38.0 },
            ClaimPoint { x: 24.0, y: 73.0 },
            ClaimPoint { x: 55.0, y: 86.0 },
            ClaimPoint { x: 83.0, y: 68.0 },
            ClaimPoint { x: 80.0, y: 50.0 },
        ];
        let mark = ClaimMark::from_canvas_stroke(&stroke, 100.0, 100.0).expect("loop");
        assert_eq!(mark.kind(), ClaimMarkKind::Drawn);
        assert_eq!(mark.quantized_points().len(), 64);
        assert_eq!(
            ClaimMark::from_canonical_bytes(&mark.canonical_bytes()).expect("decode"),
            mark
        );
    }

    #[test]
    fn tap_line_open_and_non_enclosing_paths_fail() {
        assert_eq!(
            ClaimMark::from_canvas_stroke(&[ClaimPoint { x: 50.0, y: 50.0 }], 100.0, 100.0),
            Err(ClaimMarkError::TooShort)
        );
        assert_eq!(
            ClaimMark::from_canvas_stroke(
                &[
                    ClaimPoint { x: 10.0, y: 10.0 },
                    ClaimPoint { x: 30.0, y: 30.0 },
                    ClaimPoint { x: 60.0, y: 60.0 },
                    ClaimPoint { x: 90.0, y: 90.0 },
                ],
                100.0,
                100.0
            ),
            Err(ClaimMarkError::OpenStroke)
        );
        assert_eq!(
            ClaimMark::from_canvas_stroke(
                &[
                    ClaimPoint { x: 8.0, y: 8.0 },
                    ClaimPoint { x: 38.0, y: 8.0 },
                    ClaimPoint { x: 38.0, y: 38.0 },
                    ClaimPoint { x: 8.0, y: 38.0 },
                    ClaimPoint { x: 8.0, y: 8.0 },
                ],
                100.0,
                100.0
            ),
            Err(ClaimMarkError::DoesNotEncloseArtifact)
        );
    }

    #[test]
    fn accessibility_mark_is_fixed_and_round_trips() {
        let mark = ClaimMark::accessibility_hold();
        assert_eq!(mark.kind(), ClaimMarkKind::AccessibilityHold);
        assert_eq!(
            ClaimMark::from_canonical_bytes(&mark.canonical_bytes()).expect("decode"),
            mark
        );
        let mut altered = mark.canonical_bytes();
        *altered.last_mut().expect("last") ^= 1;
        assert_eq!(
            ClaimMark::from_canonical_bytes(&altered),
            Err(ClaimMarkError::InvalidEncoding)
        );
    }

    #[test]
    fn frozen_vectors_cover_drawn_failure_and_accessibility_paths() {
        let fixture: VectorFixture =
            serde_json::from_str(include_str!("../../fixtures/claim-mark-v1.json"))
                .expect("fixture");
        assert_eq!(fixture.schema, "tohseno.claim-mark-vectors/1");
        assert_eq!(fixture.encoding["point_count"], 64);
        assert_eq!(fixture.vectors.len(), 9);
        for vector in fixture.vectors {
            let result = if vector.kind == "accessibility_hold" {
                Ok(ClaimMark::accessibility_hold())
            } else {
                let canvas = vector.canvas.expect("drawn canvas");
                ClaimMark::from_canvas_stroke(
                    vector.points.as_deref().expect("drawn points"),
                    canvas.width,
                    canvas.height,
                )
            };
            assert_eq!(result.is_ok(), vector.accepted, "{}", vector.id);
            match result {
                Ok(mark) => {
                    assert_eq!(
                        format!("0x{}", encode_hex(&mark.canonical_bytes())),
                        vector.canonical_hex.expect("canonical hex"),
                        "{}",
                        vector.id
                    );
                    assert_eq!(
                        format!("0x{}", encode_hex(&mark.gesture_commitment())),
                        vector.gesture_commitment.expect("commitment"),
                        "{}",
                        vector.id
                    );
                    assert!(vector.error.is_none());
                }
                Err(error) => assert_eq!(
                    error_code(&error),
                    vector.error.expect("error"),
                    "{}",
                    vector.id
                ),
            }
        }
    }

    fn error_code(error: &ClaimMarkError) -> &'static str {
        match error {
            ClaimMarkError::InvalidCanvas => "invalid_canvas",
            ClaimMarkError::InvalidPoint => "invalid_point",
            ClaimMarkError::TooShort => "too_short",
            ClaimMarkError::OpenStroke => "open_stroke",
            ClaimMarkError::DoesNotEncloseArtifact => "does_not_enclose_artifact",
            ClaimMarkError::InvalidEncoding => "invalid_encoding",
        }
    }

    fn encode_hex(bytes: &[u8]) -> String {
        let mut value = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(value, "{byte:02x}").expect("hex");
        }
        value
    }
}
