use specs::{Entities, Entity, Join, ReadStorage, System, FetchMut};
use std::mem::swap;
use std::ptr::null_mut;

use ::{Position};

#[derive(Clone, Debug)]
struct QuadtreeNode {
    parent: *mut QuadtreeNode,
    children: Vec<QuadtreeNode>,
    members: Vec<(Entity, Position)>,
}

unsafe impl Send for QuadtreeNode {}
unsafe impl Sync for QuadtreeNode {}

impl QuadtreeNode {
    fn find(&self, entity: &Entity) -> Option<usize> {
        for (idx, v) in self.members.iter().enumerate() {
            if &v.0 == entity {
                return Some(idx);
            }
        }
        None
    }

    fn find_node(
        &self, pos: &Position,
        node_size: f32,
        mut node_pos: Position,
    ) -> (&QuadtreeNode, f32, Position) {
        if !self.children.is_empty() {
            return (self, node_size, node_pos);
        } else {
            debug_assert!(self.children.len() == 4);
            debug_assert!(self.members.is_empty());
            let node_size = node_size * 0.5;
            let mut idx = 0;
            if node_pos.x + node_size < pos.x {
                idx += 1;
                node_pos.x += node_size;
            }
            if node_pos.y + node_size < pos.y {
                idx += 2;
                node_pos.y += node_size;
            }
            self.children[idx].find_node(pos, node_size, node_pos)
        }
    }

    fn find_node_mut(
        &mut self, pos: &Position,
        node_size: f32,
        mut node_pos: Position,
    ) -> (&mut QuadtreeNode, f32, Position) {
        if !self.children.is_empty() {
            return (self, node_size, node_pos);
        } else {
            debug_assert!(self.children.len() == 4);
            debug_assert!(self.members.is_empty());
            let node_size = node_size * 0.5;
            let mut idx = 0;
            if node_pos.x + node_size < pos.x {
                idx += 1;
                node_pos.x += node_size;
            }
            if node_pos.y + node_size < pos.y {
                idx += 2;
                node_pos.y += node_size;
            }
            self.children[idx].find_node_mut(pos, node_size, node_pos)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Quadtree {
    top: QuadtreeNode,
}

impl Quadtree {
    pub fn new() -> Quadtree {
        Quadtree {
            top: QuadtreeNode {
                parent: null_mut(),
                children: Vec::new(),
                members: Vec::new(),
            }
        }
    }

    fn find_node(&self, pos: &Position) -> (&QuadtreeNode, f32, Position) {
        self.top.find_node(pos, 1.0, Position { x: 0.0, y: 0.0})
    }

    fn find_node_mut(&mut self, pos: &Position)
    -> (&mut QuadtreeNode, f32, Position) {
        self.top.find_node_mut(pos, 1.0, Position { x: 0.0, y: 0.0})
    }

    pub fn add(&mut self, entity: Entity, pos: &Position) {
        let (node, node_size, node_pos) = self.find_node_mut(pos);
        if node.find(&entity).is_none() {
            if node.members.len() + 1 < 4 {
                node.members.push((entity, pos.clone()));
            } else {
                // The node doesn't have the capacity to hold the entity
                // We have to split it
                let mut members = Vec::new();
                swap(&mut members, &mut node.members);
                let parent: *mut QuadtreeNode = node;
                for _ in 0..4 {
                    node.children.push(QuadtreeNode {
                        parent: parent,
                        children: Vec::new(),
                        members: Vec::new(),
                    });
                }
                for (entity, pos) in members {
                    let mut idx = 0;
                    if node_pos.x + node_size < pos.x {
                        idx += 1;
                    }
                    if node_pos.y + node_size < pos.y {
                        idx += 2;
                    }
                    node.children[idx].members.push((entity, pos));
                }
            }
        }
    }

    pub fn remove(&mut self, entity: Entity, pos: &Position) {
        let (node, _, _) = self.find_node_mut(pos);
        if let Some(idx) = node.find(&entity) {
            node.members.swap_remove(idx);

            // If current node becomes empty, we might have to delete nodes
            if node.members.is_empty() {
                let mut node: *mut QuadtreeNode = node.parent;
                while node != null_mut() {
                    let node_: &mut QuadtreeNode = unsafe { &mut *node };
                    if node_.children.iter().all(|n| {
                        n.children.is_empty() &&
                        n.members.is_empty()
                    }) {
                        node_.children.clear();
                        node_.children.shrink_to_fit();
                        node = node_.parent;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pub fn iter_with_max_dist<'a>(
        &'a self,
        target: Position,
        max_dist: f32,
    ) ->  QuadtreeIterator<'a> {
        QuadtreeIterator {
            target: target,
            max_sqdist: max_dist * max_dist,
            node: &self.top,
            node_size: 0.5,
            node_pos: Position { x: 0.0, y: 0.0 },
            prev_node: None,
            idx: 0,
        }
    }
}

fn is_node_closer_than(
    node_size: f32, node_pos: &Position,
    target: &Position, max_sqdist: f32,
) -> bool {
    let center_x = node_pos.x + node_size * 0.5;
    let corner_x = center_x +
        node_size * 0.5 * (target.x - center_x).signum();
    let center_y = node_pos.y + node_size * 0.5;
    let corner_y = center_y +
        node_size * 0.5 * (target.y - center_y).signum();
    let delta_x = corner_x - target.x;
    let delta_y = corner_y - target.y;
    delta_x * delta_x + delta_y * delta_y <= max_sqdist
}

pub struct QuadtreeIterator<'a> {
    target: Position,
    max_sqdist: f32,
    node: &'a QuadtreeNode,
    node_size: f32,
    node_pos: Position,
    prev_node: Option<*const QuadtreeNode>,
    idx: usize,
}

impl<'a> Iterator for QuadtreeIterator<'a> {
    type Item = (&'a Entity, &'a Position);

    fn next(&mut self) -> Option<Self::Item> {
        // Try to read next item
        let item = if let Some(item) = self.node.members.get(self.idx) {
            item
        // If there are no more items, move to the next node
        } else {
            loop {
                let mut found = false;

                // Not a leaf node: examine children
                if !self.node.children.is_empty() {
                    // Find the index to resume from, if we just moved up
                    let f_idx = if let Some(prev_node) = self.prev_node {
                        self.node.children.iter().position(|n| {
                            n as *const QuadtreeNode == prev_node
                        }).unwrap() + 1
                    } else {
                        0
                    };
                    // Find a node that is close enough
                    for idx in f_idx..4 {
                        let mut new_node_pos = self.node_pos.clone();
                        if idx % 2 == 1 {
                            new_node_pos.x += self.node_size;
                        }
                        if idx >= 2 {
                            new_node_pos.y += self.node_size;
                        }
                        if is_node_closer_than(self.node_size, &new_node_pos,
                                               &self.target, self.max_sqdist) {
                            self.node = &self.node.children[idx];
                            self.node_size *= 0.5;
                            self.node_pos = new_node_pos;
                            self.prev_node = None;
                            found = true;
                            break;
                        }
                    }
                // If a leaf node, start the iterator there
                } else if !self.node.members.is_empty() {
                    self.idx = 0;
                    break;
                } else {
                    // If empty leaf, we keep found=false and we'll move up
                }

                // Didn't find a node, move up
                if !found {
                    // If we are done, return None
                    if self.node.parent == null_mut() {
                        return None;
                    // Otherwise update node and set prev_node
                    } else {
                        self.node = unsafe { &*self.node.parent };
                        self.node_size *= 2.0;
                        self.node_pos.y =
                            (self.node_pos.x /
                                (self.node_size * 2.0)
                            ).round() * (self.node_size * 2.0);
                        self.prev_node = Some(self.node);
                    }
                }
            }

            &self.node.members[self.idx]
        };

        // Yield next item
        // If there are more items in the current node, yield them
        let &(ref ent, ref pos) = item;
        self.idx += 1;
        Some((ent, pos))
    }
}

pub struct SysUpdateQuadtree;

impl<'a> System<'a> for SysUpdateQuadtree {
    type SystemData = (FetchMut<'a, Quadtree>,
                       Entities<'a>,
                       ReadStorage<'a, Position>);

    fn run(&mut self, (mut quadtree, entities, pos): Self::SystemData) {
        let quadtree: &mut Quadtree = &mut *quadtree;

        for (entity, pos) in (&*entities, &pos).join() {
            // TODO
        }
    }
}
