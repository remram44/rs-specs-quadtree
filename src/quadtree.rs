use specs::{Component, Entities, Entity, FetchMut, Join, ReadStorage, System,
            VecStorage, WriteStorage};
use std::mem::swap;
use std::ptr::null_mut;

use ::{Position};

#[derive(Clone, Debug)]
pub struct Bounds {
    pub pos: Position,
    pub size: f32,
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
pub struct QuadtreeRef(*mut QuadtreeNode);

unsafe impl Send for QuadtreeRef {}
unsafe impl Sync for QuadtreeRef {}

impl Component for QuadtreeRef {
    type Storage = VecStorage<Self>;
}

#[derive(Clone, Debug)]
struct QuadtreeNode {
    bounds: Bounds,
    parent: *mut QuadtreeNode,
    children: Vec<QuadtreeNode>,
    members: Vec<(Entity, Bounds)>,
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

    fn find_node(&self, bounds: &Bounds) -> &QuadtreeNode {
        if !self.children.is_empty() {
            debug_assert!(self.children.len() == 4);
            let half_size = self.bounds.size * 0.5;
            let mut idx = 0;
            let mid_x = self.bounds.pos.x + half_size;
            // It fits on the right half
            if mid_x < bounds.pos.x {
                idx += 1;
            // It doesn't fit on either half
            } else if mid_x < bounds.pos.x + bounds.size {
                return self;
            // Else, it fits on the left half
            }
            let mid_y = self.bounds.pos.y + half_size;
            // It fits on the top half
            if mid_y < bounds.pos.y {
                idx += 2;
            // It doesn't fit on either half
            } else if mid_y < bounds.pos.y + bounds.size {
                return self;
            // Else, it fits on the botton half
            }
            return self.children[idx].find_node(bounds);
        }
        self
    }

    fn find_node_mut(&mut self, bounds: &Bounds) -> &mut QuadtreeNode {
        if !self.children.is_empty() {
            debug_assert!(self.children.len() == 4);
            let half_size = self.bounds.size * 0.5;
            let mut idx = 0;
            let mid_x = self.bounds.pos.x + half_size;
            // It fits on the right half
            if mid_x < bounds.pos.x {
                idx += 1;
            // It doesn't fit on either half
            } else if mid_x < bounds.pos.x + bounds.size {
                return self;
            // Else, it fits on the left half
            }
            let mid_y = self.bounds.pos.y + half_size;
            // It fits on the top half
            if mid_y < bounds.pos.y {
                idx += 2;
            // It doesn't fit on either half
            } else if mid_y < bounds.pos.y + bounds.size {
                return self;
            // Else, it fits on the botton half
            }
            return self.children[idx].find_node_mut(bounds);
        }
        self
    }

    pub fn add(&mut self, entity: Entity, bounds: Bounds) {
        if self.members.len() < 4 {
            self.members.push((entity, bounds.clone()));
        } else {
            // The node doesn't have the capacity to hold the entity
            // We have to split it
            let mut members = Vec::new();
            swap(&mut members, &mut self.members);
            let parent: *mut QuadtreeNode = self;
            for idx in 0..4 {
                self.children.push(QuadtreeNode {
                    bounds: self.bounds.split(idx),
                    parent: parent,
                    children: Vec::new(),
                    members: Vec::new(),
                });
            }
            for (old_entity, old_bounds) in members {
                self.find_node_mut(&old_bounds).members
                    .push((old_entity, old_bounds));
            }
            self.find_node_mut(&bounds).members.push((entity, bounds));
        }
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(idx) = self.find(&entity) {
            self.members.swap_remove(idx);

            // If current node becomes empty, we might have to delete nodes
            if self.members.is_empty() {
                let mut node: *mut QuadtreeNode = self.parent;
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

    // FIXME: unused?
    pub fn _add(&mut self, entity: Entity, bounds: &Bounds) {
        let node = self.top.find_node_mut(bounds);
        if node.find(&entity).is_none() {
            node.add(entity, bounds.clone());
        }
    }

    // FIXME: unused?
    pub fn _remove(&mut self, entity: Entity, bounds: &Bounds) {
        let node = self.top.find_node_mut(bounds);
        if let Some(idx) = node.find(&entity) {
            node.members.swap_remove(idx);
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
    type Item = (&'a Entity, &'a Bounds);

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
        let &(ref ent, ref bounds) = item;
        self.idx += 1;
        Some((ent, bounds))
    }
}

pub struct SysUpdateQuadtree;

impl<'a> System<'a> for SysUpdateQuadtree {
    type SystemData = (WriteStorage<'a, QuadtreeRef>,
                       FetchMut<'a, Quadtree>,
                       Entities<'a>,
                       ReadStorage<'a, Bounds>);

    fn run(
        &mut self,
        (mut refs, mut quadtree, entities, bounds): Self::SystemData
    ) {
        let quadtree: &mut Quadtree = &mut *quadtree;

        for (entity, bounds) in (&*entities, &bounds).join() {
            let half_size = bounds.size * 0.5;
            println!("gotta update entity {:?} at {}, {}",
                     entity,
                     bounds.pos.x + half_size, bounds.pos.y + half_size);

            if let Some(quadref) = refs.get_mut(entity) {
                // Check that it still fits
                let node = unsafe { &mut *quadref.0 };
                if node.bounds.pos.x < bounds.pos.x &&
                    bounds.pos.x + bounds.size <
                        node.bounds.pos.x + node.bounds.size &&
                    node.bounds.pos.y < bounds.pos.y &&
                    bounds.pos.y + bounds.size <
                        node.bounds.pos.y + node.bounds.size {
                    // Check whether it could fit in one of the children
                    if {
                        let ptr: *const QuadtreeNode = node;
                        let better_child = node.find_node_mut(bounds);
                        let better_ptr: *const QuadtreeNode = better_child;
                        if better_ptr != ptr {
                            println!("Moving it to children node {}, {}, {}",
                                     better_child.bounds.pos.x,
                                     better_child.bounds.pos.y,
                                     better_child.bounds.size);
                            better_child.add(entity, bounds.clone());
                            true
                        } else {
                            false // This is the best place for the entity
                        }
                    } {
                        // We defer the remove call after the borrow ends
                        node.remove(entity);
                    } else {
                        println!("Still in best node {}, {}, {}",
                                 node.bounds.pos.x,
                                 node.bounds.pos.y,
                                 node.bounds.size);
                    }
                } else {
                    // Find the new correct position for this entity
                    node.remove(entity);
                    let new_node = quadtree.top.find_node_mut(bounds);
                    println!("Moving it to node {}, {}, {}",
                             new_node.bounds.pos.x,
                             new_node.bounds.pos.y,
                             new_node.bounds.size);
                    new_node.add(entity, bounds.clone());
                }
            // If it's not in the quadtree yet, just add it
            } else {
                let node = quadtree.top.find_node_mut(bounds);
                println!("Not yet in quadtree, adding to node {}, {}, {}",
                         node.bounds.pos.x, node.bounds.pos.y,
                         node.bounds.size);
                node.add(entity, bounds.clone());
            }
        }
    }
}
