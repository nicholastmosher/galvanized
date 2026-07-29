use gpui::{AnyElement, App, Axis, Entity, IntoElement, ParentElement, Window};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};

use crate::pane::Pane;

/// Direction for splitting a pane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SplitDirection {
    pub fn axis(self) -> Axis {
        match self {
            SplitDirection::Up | SplitDirection::Down => Axis::Vertical,
            SplitDirection::Left | SplitDirection::Right => Axis::Horizontal,
        }
    }

    pub fn increasing(self) -> bool {
        match self {
            SplitDirection::Up | SplitDirection::Left => false,
            SplitDirection::Down | SplitDirection::Right => true,
        }
    }
}

/// A member of a PaneGroup tree: either a single Pane or a nested Axis split.
#[derive(Clone)]
pub enum Member {
    Pane(Entity<Pane>),
    Axis(PaneAxis),
}

impl Member {
    fn new_axis(old_pane: Entity<Pane>, new_pane: Entity<Pane>, direction: SplitDirection) -> Self {
        let axis = direction.axis();
        let members = match direction {
            SplitDirection::Up | SplitDirection::Left => {
                vec![Member::Pane(new_pane), Member::Pane(old_pane)]
            }
            SplitDirection::Down | SplitDirection::Right => {
                vec![Member::Pane(old_pane), Member::Pane(new_pane)]
            }
        };
        Member::Axis(PaneAxis::new(axis, members))
    }

    fn first_pane(&self) -> Entity<Pane> {
        match self {
            Member::Axis(axis) => axis.members[0].first_pane(),
            Member::Pane(pane) => pane.clone(),
        }
    }

    fn collect_panes<'a>(&'a self, panes: &mut Vec<&'a Entity<Pane>>) {
        match self {
            Member::Axis(axis) => {
                for member in &axis.members {
                    member.collect_panes(panes);
                }
            }
            Member::Pane(pane) => panes.push(pane),
        }
    }

    fn render(&self, _basis: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        match self {
            Member::Pane(pane) => pane.clone().into_any_element(),
            Member::Axis(axis) => axis.render(window, cx),
        }
    }
}

/// A split axis containing multiple members arranged along an axis.
#[derive(Clone)]
pub struct PaneAxis {
    pub axis: Axis,
    pub members: Vec<Member>,
}

impl PaneAxis {
    pub fn new(axis: Axis, members: Vec<Member>) -> Self {
        Self { axis, members }
    }

    fn split(
        &mut self,
        old_pane: &Entity<Pane>,
        new_pane: &Entity<Pane>,
        direction: SplitDirection,
    ) -> bool {
        for (mut idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if axis.split(old_pane, new_pane, direction) {
                        return true;
                    }
                }
                Member::Pane(pane) => {
                    if pane == old_pane {
                        if direction.axis() == self.axis {
                            if direction.increasing() {
                                idx += 1;
                            }
                            self.members.insert(idx, Member::Pane(new_pane.clone()));
                        } else {
                            *member =
                                Member::new_axis(old_pane.clone(), new_pane.clone(), direction);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    fn remove(&mut self, pane_to_remove: &Entity<Pane>) -> Option<Member> {
        let mut found = false;
        let mut remove_idx = None;

        for (idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if let Some(last_pane) = axis.remove(pane_to_remove) {
                        *member = last_pane;
                        found = true;
                        break;
                    }
                }
                Member::Pane(pane) => {
                    if pane == pane_to_remove {
                        found = true;
                        remove_idx = Some(idx);
                        break;
                    }
                }
            }
        }

        if found {
            if let Some(idx) = remove_idx {
                self.members.remove(idx);
            }

            if self.members.len() == 1 {
                self.members.pop()
            } else {
                None
            }
        } else {
            None
        }
    }

    fn render(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let group_id = format!("pane-axis-{:p}", self as *const Self);

        let mut group = if self.axis == Axis::Horizontal {
            h_resizable(group_id)
        } else {
            v_resizable(group_id)
        };

        for member in &self.members {
            let child = member.render(0, window, cx);
            group = group.child(resizable_panel().child(child));
        }

        group.into_any_element()
    }
}

/// A tree of panes that supports splitting and removing panes.
pub struct PaneGroup {
    pub root: Member,
}

impl PaneGroup {
    pub fn new(pane: Entity<Pane>) -> Self {
        Self {
            root: Member::Pane(pane),
        }
    }

    pub fn split(
        &mut self,
        old_pane: &Entity<Pane>,
        new_pane: &Entity<Pane>,
        direction: SplitDirection,
    ) {
        let found = match &mut self.root {
            Member::Pane(pane) => {
                if pane == old_pane {
                    self.root = Member::new_axis(old_pane.clone(), new_pane.clone(), direction);
                    true
                } else {
                    false
                }
            }
            Member::Axis(axis) => axis.split(old_pane, new_pane, direction),
        };

        if !found {
            let first_pane = self.root.first_pane();
            match &mut self.root {
                Member::Pane(_) => {
                    self.root = Member::new_axis(first_pane, new_pane.clone(), direction);
                }
                Member::Axis(axis) => {
                    let _ = axis.split(&first_pane, new_pane, direction);
                }
            }
        }
    }

    pub fn remove(&mut self, pane: &Entity<Pane>) {
        match &mut self.root {
            Member::Pane(_) => {}
            Member::Axis(axis) => {
                if let Some(last_pane) = axis.remove(pane) {
                    self.root = last_pane;
                }
            }
        }
    }

    pub fn panes(&self) -> Vec<&Entity<Pane>> {
        let mut panes = Vec::new();
        self.root.collect_panes(&mut panes);
        panes
    }

    pub fn first_pane(&self) -> Entity<Pane> {
        self.root.first_pane()
    }

    pub fn render(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        self.root.render(0, window, cx)
    }
}
