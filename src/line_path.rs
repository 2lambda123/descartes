use {N, P2, V2, VecLike};
use rough_eq::{RoughEq, THICKNESS};
use angles::WithUniqueOrthogonal;
use ordered_float::OrderedFloat;
use segments::LineSegment;

#[cfg_attr(feature = "compact_containers", derive(Compact))]
#[cfg_attr(feature = "serde-serialization", derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
#[derive(Clone, Debug)]
pub struct LinePath {
    pub points: VecLike<P2>,
    pub distances: VecLike<N>,
}

/// Creation
impl LinePath {
    pub fn new(mut points: VecLike<P2>) -> Option<Self> {
        if points.len() >= 2 {
            let old_last = *points.last().unwrap();
            let mut previous_point = points[0];
            let mut distances = VecLike::with_capacity(points.len());
            let mut current_distance = 0.0;
            let mut is_first = true;

            points.retain(|point| {
                let new_distance = (point - previous_point).norm();

                if is_first || new_distance > THICKNESS {
                    current_distance += new_distance;
                    distances.push(current_distance);
                    previous_point = *point;
                    is_first = false;
                    true
                } else {
                    false
                }
            });

            if points.len() >= 2 {
                // special case: path got shortened because last point is too close to previous
                // make sure that we keep the last point instead of the previous
                let update_last_distance = {
                    let last = points.last_mut().unwrap();
                    if *last != old_last {
                        *last = old_last;
                        true
                    } else {
                        false
                    }
                };

                if update_last_distance {
                    let previous_distance = distances[distances.len() - 2];
                    let previous_point = points[points.len() - 2];
                    *distances.last_mut().unwrap() =
                        previous_distance + (old_last - previous_point).norm()
                }

                Some(LinePath { points, distances })
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[test]
fn constructor() {
    assert_eq!(
        LinePath::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
        ]),
        Some(LinePath {
            points: vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)],
            distances: vec![0.0, 1.0, 2.0],
        })
    );
}

#[test]
fn constructor_simplify_points() {
    assert_eq!(
        LinePath::new(vec![
            P2::new(0.0, 0.0),
            P2::new(0.0 + THICKNESS / 2.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0 + THICKNESS / 2.0, 0.0),
            P2::new(1.0 - THICKNESS / 2.0, 1.0),
            P2::new(1.0, 1.0),
        ]),
        Some(LinePath {
            points: vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)],
            distances: vec![0.0, 1.0, 2.0],
        })
    );
}

#[test]
fn constructor_invalid() {
    assert_eq!(
        LinePath::new(vec![P2::new(0.0, 0.0), P2::new(0.0 + THICKNESS / 2.0, 0.0)]),
        None
    );
}

/// Inspection
impl LinePath {
    pub fn start(&self) -> P2 {
        self.points[0]
    }

    pub fn end(&self) -> P2 {
        *self.points.last().unwrap()
    }

    pub fn length(&self) -> N {
        *self.distances.last().unwrap()
    }

    pub fn segments<'a>(&'a self) -> impl Iterator<Item = LineSegment> + 'a {
        self.points
            .windows(2)
            .map(|window| LineSegment::new(window[0], window[1]))
    }

    pub fn segments_with_distances(&self) -> impl Iterator<Item = (LineSegment, &[N])> {
        self.segments().zip(self.distances.windows(2))
    }

    pub fn first_segment(&self) -> LineSegment {
        LineSegment::new(self.points[0], self.points[1])
    }

    pub fn last_segment(&self) -> LineSegment {
        let last = self.points.len() - 1;
        LineSegment::new(self.points[last - 1], self.points[last])
    }

    pub fn nth_segment(&self, n: usize) -> LineSegment {
        LineSegment::new(self.points[n], self.points[n + 1])
    }

    pub fn find_on_segment(&self, distance: N) -> Option<(LineSegment, N)> {
        for (segment, distance_pair) in self.segments_with_distances() {
            if distance_pair[1] >= distance {
                return Some((segment, distance - distance_pair[0]));
            }
        }
        None
    }

    pub fn along(&self, distance: N) -> P2 {
        if let Some((segment, distance_on_segment)) = self.find_on_segment(distance) {
            segment.along(distance_on_segment)
        } else if distance < 0.0 {
            self.start()
        } else {
            self.end()
        }
    }

    pub fn direction_along(&self, distance: N) -> V2 {
        if let Some((segment, _)) = self.find_on_segment(distance) {
            segment.direction()
        } else if distance < 0.0 {
            self.first_segment().direction()
        } else {
            self.last_segment().direction()
        }
    }

    pub fn start_direction(&self) -> V2 {
        self.first_segment().direction()
    }

    pub fn end_direction(&self) -> V2 {
        self.last_segment().direction()
    }

    pub fn project_with_tolerance(&self, point: P2, tolerance: N) -> Option<(N, P2)> {
        self.segments_with_distances()
            .filter_map(|(segment, distances)| {
                segment.project_with_tolerance(point, tolerance).map(
                    |(distance_on_segment, projected_point)| {
                        (distance_on_segment + distances[0], projected_point)
                    },
                )
            })
            .min_by_key(|(_, projected_point)| OrderedFloat((projected_point - point).norm()))
    }

    pub fn project_with_max_distance(
        &self,
        point: P2,
        tolerance: N,
        max_distance: N,
    ) -> Option<(N, P2)> {
        self.project_with_tolerance(point, tolerance)
            .and_then(|(along, projected_point)| {
                if (projected_point - point).norm() <= max_distance {
                    Some((along, projected_point))
                } else {
                    None
                }
            })
    }

    pub fn project(&self, point: P2) -> Option<(N, P2)> {
        self.project_with_tolerance(point, THICKNESS)
    }

    pub fn includes(&self, point: P2) -> bool {
        self.distance_to(point) < THICKNESS
    }

    pub fn distance_to(&self, point: P2) -> N {
        if let Some((_, projected_point)) = self.project(point) {
            (point - projected_point).norm()
        } else {
            *::std::cmp::min(
                OrderedFloat((point - self.start()).norm()),
                OrderedFloat((point - self.end()).norm()),
            )
        }
    }
}

#[derive(Debug)]
pub enum ConcatError {
    PointsTooFarApart,
    CreatedInvalidSegment
}

// TODO: this is a super hacky newtype to avoid weird problems with impl Iterator<Item = V2>
#[derive(Copy, Clone, Debug)]
pub struct ShiftVector(pub V2);

/// Combination/Modification
impl LinePath {
    pub fn concat(&self, other: &Self) -> Result<Self, ConcatError> {
        self.concat_weld(other, THICKNESS)
    }

    pub fn concat_weld(&self, other: &Self, tolerance: N) -> Result<Self, ConcatError> {
        if other.start().rough_eq_by(self.end(), tolerance) {
            LinePath::new(
                self.points
                    .iter()
                    .chain(other.points.iter())
                    .cloned()
                    .collect(),
            ).ok_or(ConcatError::CreatedInvalidSegment)
        } else {
            Err(ConcatError::PointsTooFarApart)
        }
    }

    pub fn reverse(&self) -> Self {
        let mut points = self.points.clone();
        points.reverse();
        LinePath::new(points).expect("Reversing should always work")
    }

    pub fn subsection(&self, start: N, end: N) -> Option<Self> {
        LinePath::new(
            Some(self.along(start))
                .into_iter()
                .chain(self.points.iter().zip(self.distances.iter()).filter_map(
                    |(&point, &distance)| {
                        if start < distance && end > distance {
                            Some(point)
                        } else {
                            None
                        }
                    },
                ))
                .chain(Some(self.along(end)))
                .collect(),
        )
    }

    pub fn dash(&self, dash_length: N, gap_length: N) -> DashIterator {
        DashIterator {
            path: self,
            dash_length,
            gap_length,
            position: 0.0
        }
    }

    pub fn shift_orthogonally_vectors<'a>(&'a self) -> impl Iterator<Item = ShiftVector> + 'a {
        fn bisector(a: P2, b: P2, c: P2) -> ShiftVector {
            let bisecting_direction = (LineSegment::new(a, b).direction()
                + LineSegment::new(b, c).direction())
                .orthogonal_right()
                .normalize();
            let amount_too_short = LineSegment::new(a, b).direction().orthogonal_right().dot(&bisecting_direction);
            ShiftVector(bisecting_direction * (1.0 / amount_too_short))
        }
        let angle_bisectors = self.points.windows(3).map(|triplet| bisector(triplet[0], triplet[1], triplet[2]));

        let (first_vector, last_vector) = if self.points.len() >= 3 && self.start().rough_eq(self.end()) {
            let first_vector = bisector(self.points[self.points.len() - 2], self.points[0], self.points[1]);
            (first_vector, first_vector)
        } else {
            let first_vector: ShiftVector = ShiftVector(self.first_segment().direction().orthogonal_right());
            let last_vector: ShiftVector = ShiftVector(self.last_segment().direction().orthogonal_right());
            (first_vector, last_vector)
        };

        Some(first_vector)
            .into_iter()
            .chain(angle_bisectors)
            .chain(Some(last_vector))
    }

    pub fn shift_orthogonally(&self, shift_to_right: N) -> Option<Self> {
        let new_points = self
            .points
            .iter()
            .zip(self.shift_orthogonally_vectors())
            .map(|(point, shift_vector)| point + shift_to_right * shift_vector.0)
            .collect();

        LinePath::new(new_points)
    }

    pub fn with_new_start_and_end(&self, new_start: P2, new_end: P2) -> Option<Self> {
        let last = self.points.len() - 1;
        Self::new(
            Some(new_start)
                .into_iter()
                .chain(self.points[1..last].iter().cloned())
                .chain(Some(new_end))
                .collect(),
        )
    }
}

pub struct DashIterator<'a> {
    path: &'a LinePath,
    dash_length: N,
    gap_length: N,
    position: N
}

impl<'a> Iterator for DashIterator<'a> {
    type Item = Option<LinePath>;

    fn next(&mut self) -> Option<Option<LinePath>> {
        if self.position < self.path.length() {
            let old_position = self.position;
            self.position += self.dash_length;
            let dash = self.path.subsection(old_position, self.position);
            self.position += self.gap_length;
            Some(dash)
        } else {
            None
        }
    }
}

impl<'a> RoughEq for &'a LinePath {
    fn rough_eq_by(&self, other: Self, tolerance: N) -> bool {
        self.points.len() == other.points.len()
            && self
                .points
                .iter()
                .zip(other.points.iter())
                .all(|(a, b)| a.rough_eq_by(*b, tolerance))
    }
}
