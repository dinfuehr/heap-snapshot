use crate::print::format_count;
use crate::snapshot::{HeapSnapshot, NO_DISTANCE};

use super::super::types::*;
use super::super::{App, contains_ignore_case};

impl App {
    pub(in crate::tui) fn flatten_summary(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        let filter = &self.summary_filter;
        let unreachable_only = self.summary_unreachable_only;
        for (i, agg) in self.sorted_aggregates.iter().enumerate() {
            // Skip reachable groups when unreachable-only filter is active
            if unreachable_only && agg.distance != NO_DISTANCE {
                continue;
            }

            let group_matches = filter.is_empty() || contains_ignore_case(&agg.name, filter);
            if !group_matches && !unreachable_only {
                // Group name didn't match — check if any member matches
                let any_member_match = agg
                    .node_ordinals
                    .iter()
                    .any(|ord| contains_ignore_case(snap.node_raw_name(*ord), filter));
                if !any_member_match {
                    continue;
                }
            }

            let id = self.summary_ids[i];
            let is_expanded = state.expanded.contains(&id);
            let has_children = !agg.node_ordinals.is_empty();
            let (display_count, shallow_size, retained_size) = if unreachable_only || !group_matches
            {
                let mut count = 0u32;
                let mut shallow = 0.0f64;
                let mut retained = 0.0f64;
                for ord in &agg.node_ordinals {
                    if unreachable_only && snap.node_distance(*ord) != NO_DISTANCE {
                        continue;
                    }
                    if !group_matches && !contains_ignore_case(snap.node_raw_name(*ord), filter) {
                        continue;
                    }
                    count += 1;
                    shallow += snap.node_self_size(*ord) as f64;
                    retained += snap.node_retained_size(*ord);
                }
                (count, shallow, retained)
            } else {
                (agg.count, agg.self_size, agg.max_ret)
            };
            // Skip groups with no matching members after filtering
            if display_count == 0 {
                continue;
            }
            let count_str = format!("\u{00d7}{}", format_count(display_count));
            let label = format!("{}  {count_str}", agg.name);

            rows.push(FlatRow {
                nav: FlatRowNav {
                    id,
                    parent_row: None,
                    depth: 0,
                    has_children,
                    is_expanded,
                    children_key: if has_children {
                        Some(ChildrenKey::ClassMembers(i))
                    } else {
                        None
                    },
                },
                render: FlatRowRender {
                    label: label.into(),
                    is_weak: false,
                    is_root_holder: false,
                    kind: FlatRowKind::SummaryGroup {
                        distance: agg.distance,
                        shallow_size,
                        retained_size,
                    },
                },
            });

            if is_expanded {
                let parent_row = rows.len() - 1;
                self.flatten_children(
                    &ChildrenKey::ClassMembers(i),
                    Some(parent_row),
                    1,
                    state,
                    rows,
                    snap,
                );
            }
        }
    }
}
