use specs::{Component, Entities, Entity, FetchMut, Join, ReadStorage, System,
            VecStorage};
use std::mem::swap;
use std::ptr::null_mut;

use ::{Position};

#[derive(Clone, Debug)]
struct Bounds {
    pos: Position,
    size: f32,
}

impl Bounds {
    fn split(&self, idx: usize) -> Bounds {
        let mut bounds = self.clone();
        bounds.size *= 0.5;
        if idx % 2 == 1 {
            bounds.pos.x += bounds.size;
        }
        if idx >= 2 {
            bounds.pos.y += bounds.size;
        }
        bounds
    }

    fn min_sq_dist(&self, target: &Position) -> f32 {
        let half_size = self.size * 0.5;
        let center_x = self.pos.x + half_size;
        let corner_x = center_x + half_size * (target.x - center_x).signum();
        let center_y = self.pos.y + half_size;
        let corner_y = center_y + half_size * (target.y - center_y).signum();
        let delta_x = corner_x - target.x;
        let delta_y = corner_y - target.y;
        delta_x * delta_x + delta_y * delta_y
    }
}

impl Component for Bounds {
    type Storage = VecStorage<Self>;
}

#[derive(Clone, Debug)]
struct QuadtreeNode {
    bounds: Bounds,
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

    fn find_node(&self, pos: &Position) -> &QuadtreeNode {
        if !self.children.is_empty() {
            return self;
        } else {
            debug_assert!(self.children.len() == 4);
            debug_assert!(self.members.is_empty());
            let half_size = self.bounds.size * 0.5;
            let mut idx = 0;
            if self.bounds.pos.x + half_size < pos.x {
                idx += 1;
            }
            if self.bounds.pos.y + half_size < pos.y {
                idx += 2;
            }
            self.children[idx].find_node(pos)
        }
    }

    fn find_node_mut(&mut self, pos: &Position) -> &mut QuadtreeNode {
        if !self.children.is_empty() {
            return self;
        } else {
            debug_assert!(self.children.len() == 4);
            debug_assert!(self.members.is_empty());
            let half_size = self.bounds.size * 0.5;
            let mut idx = 0;
            if self.bounds.pos.x + half_size < pos.x {
                idx += 1;
            }
            if self.bounds.pos.y + half_size < pos.y {
                idx += 2;
            }
            self.children[idx].find_node_mut(pos)
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
                bounds: Bounds {
                    pos: Position { x: 0.0, y: 0.0 },
                    size: 1.0,
                },
                parent: null_mut(),
                children: Vec::new(),
                members: Vec::new(),
            }
        }
    }

    pub fn add(&mut self, entity: Entity, pos: &Position) {
        let node = self.top.find_node_mut(pos);
        if node.find(&entity).is_none() {
            if node.members.len() < 4 {
                node.members.push((entity, pos.clone()));
            } else {
                // The node doesn't have the capacity to hold the entity
                // We have to split it
                let mut members = Vec::new();
                swap(&mut members, &mut node.members);
                let parent: *mut QuadtreeNode = node;
                for idx in 0..4 {
                    node.children.push(QuadtreeNode {
                        bounds: node.bounds.split(idx),
                        parent: parent,
                        children: Vec::new(),
                        members: Vec::new(),
                    });
                }
                for (entity, pos) in members {
                    let half_size = node.bounds.size * 0.5;
                    let mut idx = 0;
                    if node.bounds.pos.x + half_size < pos.x {
                        idx += 1;
                    }
                    if node.bounds.pos.y + half_size < pos.y {
                        idx += 2;
                    }
                    node.children[idx].members.push((entity, pos));
                }
            }
        }
    }

    pub fn remove(&mut self, entity: Entity, pos: &Position) {
        let node = self.top.find_node_mut(pos);
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
            prev_node: None,
            idx: 0,
        }
    }
}

pub struct QuadtreeIterator<'a> {
    target: Position,
    max_sqdist: f32,
    node: &'a QuadtreeNode,
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
                        let child = &self.node.children[idx];
                        let minsqdist = child.bounds.min_sq_dist(&self.target);
                        if minsqdist < self.max_sqdist {
                            self.node = child;
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
            println!("gotta update entity {:?} at {}, {}",
                     entity, pos.x, pos.y);
            // TODO
        }
    }
}
