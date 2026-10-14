//! Commit-graph lane assignment — the `GTGitCommitGraphDrawer` counterpart
//! (`EV:types`: `GTGitCommitGraphDrawer`, `GTGitCommitGraphLane`,
//! `commitGraphLaneWithLaneID:startingIndex:endingIndex:starting:ending:`;
//! docs/04 §2.5).
//!
//! The evidence proves lanes are computed engine-side (`EV:analysis/
//! git-semantics#F7`); the algorithm itself is ours and pure: it takes the rows
//! of one loaded page (identity + parents, in display order) and returns the
//! lane of each row plus the strokes between them. No repository access, no
//! second walk — [`super::history`] already has the parents of every row it
//! returns.
//!
//! **Cost.** One pass over the page; each parent lookup scans the active-lane
//! vector, which is bounded by the widest part of the DAG intersecting the page
//! (tens of lanes in real repositories, not thousands).

use std::collections::HashMap;

use crate::domain::commit::{GraphEdgeKind, GraphLaneSegment, GraphLayout};

/// Minimal input for [`draw_graph`]: what the algorithm actually reads.
///
/// `parents` is in commit order, so `parents[0]` is the first parent. Parents
/// outside the loaded page are kept — they become lines that continue below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphNode {
    pub oid: String,
    pub parents: Vec<String>,
}

impl GraphNode {
    pub fn new(oid: impl Into<String>, parents: Vec<String>) -> Self {
        Self {
            oid: oid.into(),
            parents,
        }
    }
}

/// What a lane is currently doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    /// The lane is free and may host the next head or merge arm.
    Free,
    /// The lane's line terminates at this row (a bubble inside the page).
    Awaiting(usize),
    /// The lane's line continues below the loaded page: its commit is older
    /// than the last row and was not loaded.
    Below,
}

/// Compute the lane layout for one page of rows.
///
/// Rows are expected in display order (a commit before its parents). A row
/// list that violates that order still produces a drawing — lanes simply start
/// where they are first seen — but the result is only meaningful for a
/// walk-ordered page.
pub fn draw_graph(nodes: &[GraphNode]) -> GraphLayout {
    if nodes.is_empty() {
        return GraphLayout::default();
    }
    let last_row = nodes.len() as i64 - 1;

    let index_of: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.oid.as_str(), index))
        .collect();

    let mut pending: Vec<Wait> = Vec::new();
    let mut open: Vec<Vec<usize>> = Vec::new();
    let mut below_by_oid: HashMap<&str, usize> = HashMap::new();
    let mut segments: Vec<GraphLaneSegment> = Vec::new();
    let mut lanes: Vec<i32> = Vec::with_capacity(nodes.len());
    let mut lane_count: u16 = 0;

    for (row, node) in nodes.iter().enumerate() {
        let row_index = row as i64;

        // Collect the lanes waiting for this very commit.
        let mut awaiting: Vec<usize> = pending
            .iter()
            .enumerate()
            .filter(|(_, wait)| **wait == Wait::Awaiting(row))
            .map(|(lane, _)| lane)
            .collect();

        let lane = if let Some(primary) = awaiting.first().copied() {
            // Any further lane that also expected this commit is a duplicate
            // line: fold it into the primary lane at this row.
            for extra in awaiting.drain(1..) {
                close_lane(&mut segments, &mut open, extra, row_index);
                pending[extra] = Wait::Free;
            }
            close_lane(&mut segments, &mut open, primary, row_index);
            pending[primary] = Wait::Free;
            primary
        } else {
            // A head inside the page: take the lowest free lane.
            let lane = lowest_free(&pending);
            ensure_lane(&mut pending, &mut open, lane);
            lane
        };
        lane_count = lane_count.max(lane as u16 + 1);
        lanes.push(lane as i32);

        // Place the parents. The first parent continues this row's lane; every
        // further parent opens a line (or joins one that already exists).
        for (parent_index, parent) in node.parents.iter().enumerate() {
            let target = index_of.get(parent.as_str()).copied();
            let existing = match target {
                Some(index) => pending
                    .iter()
                    .position(|wait| *wait == Wait::Awaiting(index)),
                None => below_by_oid.get(parent.as_str()).copied(),
            };

            let kind = match (parent_index, existing.is_some()) {
                (0, false) => GraphEdgeKind::Straight,
                (0, true) => GraphEdgeKind::Bend,
                (_, false) => GraphEdgeKind::Merge,
                (_, true) => GraphEdgeKind::Fork,
            };

            let target_lane = match existing {
                Some(lane) => lane,
                None => {
                    let lane = if parent_index == 0 {
                        lane
                    } else {
                        lowest_free(&pending)
                    };
                    ensure_lane(&mut pending, &mut open, lane);
                    pending[lane] = match target {
                        Some(index) => Wait::Awaiting(index),
                        None => {
                            below_by_oid.insert(parent.as_str(), lane);
                            Wait::Below
                        }
                    };
                    lane
                }
            };

            lane_count = lane_count.max(target_lane as u16 + 1);
            let segment = segments.len();
            segments.push(GraphLaneSegment {
                from_row: row_index,
                // Filled in when the stroke closes; a stroke that never closes
                // is clamped to the last row at the end of the pass.
                to_row: -1,
                lane: lane as u16,
                from_lane: lane as u16,
                to_lane: target_lane as u16,
                kind,
                // Merge arms open a fresh lane at this row.
                starting: existing.is_none() && parent_index > 0,
                ending: false,
            });
            if existing.is_some() {
                // A line joining this lane terminates together with every
                // other line already converging on the same commit.
                open[target_lane].push(segment);
            } else {
                open[target_lane] = vec![segment];
            }
        }
    }

    // Lines whose commit was not loaded continue past the bottom: clamp their
    // drawing to the last row so the renderer can place the continuation
    // marker there (its `toRow` must stay inside the page).
    for segment in &mut segments {
        if segment.to_row < 0 {
            segment.to_row = last_row;
        }
    }

    GraphLayout {
        lanes,
        segments,
        lane_count,
    }
}

/// The doc-named entry point (docs/02 §13: `engine::history::draw_graph`).
pub use draw_graph as compute;

fn lowest_free(pending: &[Wait]) -> usize {
    pending
        .iter()
        .position(|wait| *wait == Wait::Free)
        .unwrap_or(pending.len())
}

fn ensure_lane(pending: &mut Vec<Wait>, open: &mut Vec<Vec<usize>>, lane: usize) {
    while pending.len() <= lane {
        pending.push(Wait::Free);
        open.push(Vec::new());
    }
}

/// Terminate every stroke a lane still has open at `row`.
fn close_lane(segments: &mut [GraphLaneSegment], open: &mut [Vec<usize>], lane: usize, row: i64) {
    if let Some(indices) = open.get_mut(lane) {
        for &index in indices.iter() {
            if let Some(segment) = segments.get_mut(index) {
                segment.to_row = row;
                segment.ending = true;
            }
        }
        indices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(oid: &str, parents: &[&str]) -> GraphNode {
        GraphNode::new(oid, parents.iter().map(|p| p.to_string()).collect())
    }

    #[test]
    fn linear_history_stays_in_lane_zero() {
        let layout = draw_graph(&[node("c", &["b"]), node("b", &["a"]), node("a", &[])]);
        assert_eq!(layout.lanes, vec![0, 0, 0]);
        assert_eq!(layout.lane_count, 1);
        // Two straight strokes, both terminating inside the page.
        assert_eq!(layout.segments.len(), 2);
        for segment in &layout.segments {
            assert_eq!(segment.kind, GraphEdgeKind::Straight);
            assert_eq!(segment.from_lane, segment.to_lane);
            assert!(segment.ending && segment.to_row >= 0);
            assert!(!segment.starting);
        }
        assert_eq!(layout.segments[0].from_row, 0);
        assert_eq!(layout.segments[0].to_row, 1);
        assert_eq!(layout.segments[1].to_row, 2);
    }

    #[test]
    fn merge_commit_opens_a_second_lane() {
        // m (merge of a and b), then a, then b.
        let layout = draw_graph(&[node("m", &["a", "b"]), node("a", &[]), node("b", &[])]);
        assert_eq!(layout.lanes, vec![0, 0, 1]);
        assert_eq!(layout.lane_count, 2);
        let merge = layout
            .segments
            .iter()
            .find(|s| s.kind == GraphEdgeKind::Merge)
            .expect("merge edge");
        assert_eq!(merge.lane, 0, "the stroke is owned by the child line");
        assert_eq!(merge.from_lane, 0);
        assert_eq!(merge.to_lane, 1);
        assert_eq!(merge.from_row, 0);
        assert_eq!(merge.to_row, 2);
        assert!(merge.starting && merge.ending);
    }

    #[test]
    fn octopus_merge_opens_one_lane_per_extra_parent() {
        let layout = draw_graph(&[
            node("o", &["a", "b", "c"]),
            node("a", &[]),
            node("b", &[]),
            node("c", &[]),
        ]);
        assert_eq!(layout.lanes, vec![0, 0, 1, 2]);
        assert_eq!(layout.lane_count, 3);
        assert_eq!(
            layout
                .segments
                .iter()
                .filter(|s| s.kind == GraphEdgeKind::Merge)
                .count(),
            2
        );
    }

    #[test]
    fn second_head_joins_the_existing_line_with_a_bend() {
        // Two heads (x, y) share one root (z): y bends into x's line.
        let layout = draw_graph(&[node("x", &["z"]), node("y", &["z"]), node("z", &[])]);
        assert_eq!(layout.lanes, vec![0, 1, 0]);
        let bend = layout
            .segments
            .iter()
            .find(|s| s.kind == GraphEdgeKind::Bend)
            .expect("bend edge");
        assert_eq!(bend.from_row, 1);
        assert_eq!(bend.to_row, 2);
        assert_eq!(bend.from_lane, 1, "leaves the second head's lane");
        assert_eq!(bend.to_lane, 0, "lands in the root's lane");
        assert!(!bend.starting && bend.ending);
    }

    #[test]
    fn merge_into_an_already_running_line_is_a_bend_from_the_second_head() {
        // m merges b, but b is already tracked by x's line.
        let layout = draw_graph(&[
            node("m", &["m1", "b"]),
            node("x", &["b"]),
            node("b", &[]),
            node("m1", &[]),
        ]);
        assert_eq!(layout.lanes, vec![0, 2, 1, 0]);
        // Both the merge arm and x's bend terminate at b's row.
        let terminating = layout.segments.iter().filter(|s| s.to_row == 2).count();
        assert_eq!(terminating, 2);
    }

    #[test]
    fn parents_below_the_page_are_clamped_to_the_last_row() {
        // The last row's parent was not loaded: the stroke is drawn down to the
        // last row (so the marker lands there) but reports `ending == false`.
        let layout = draw_graph(&[node("b", &["a"]), node("b2", &["a2"])]);
        assert_eq!(layout.lanes, vec![0, 1]);
        assert_eq!(layout.lane_count, 2);
        assert!(layout.segments.iter().all(|s| !s.ending && s.to_row == 1));
    }

    #[test]
    fn a_single_row_page_clamps_the_continuation_to_row_zero() {
        let layout = draw_graph(&[node("tip", &["older"])]);
        assert_eq!(layout.lanes, vec![0]);
        assert_eq!(layout.segments.len(), 1);
        assert_eq!(layout.segments[0].from_row, 0);
        assert_eq!(layout.segments[0].to_row, 0);
        assert!(!layout.segments[0].ending);
    }

    #[test]
    fn off_page_parents_share_one_line_per_oid() {
        // Two rows whose first parent is the same off-page commit: the second
        // row bends into the first row's lane instead of opening a new one.
        let layout = draw_graph(&[node("x", &["z"]), node("y", &["z"])]);
        assert_eq!(layout.lanes, vec![0, 1]);
        assert_eq!(
            layout
                .segments
                .iter()
                .filter(|s| s.kind == GraphEdgeKind::Bend)
                .count(),
            1
        );
        assert_eq!(layout.lane_count, 2);
    }

    #[test]
    fn empty_input_draws_nothing() {
        let layout = draw_graph(&[]);
        assert!(layout.lanes.is_empty());
        assert!(layout.segments.is_empty());
        assert_eq!(layout.lane_count, 0);
    }

    #[test]
    fn lanes_are_reused_once_a_line_ends() {
        // Two unrelated roots in one page: the first line ends at row 0, so the
        // second root may reuse lane 0 — no stroke overlaps another.
        let layout = draw_graph(&[node("a", &[]), node("b", &[])]);
        assert_eq!(layout.lanes, vec![0, 0]);
        assert_eq!(layout.lane_count, 1);
    }
}
