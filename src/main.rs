extern crate specs;

mod quadtree;

use specs::{Component, DispatcherBuilder, Join, ReadStorage,
            System, VecStorage, World, WriteStorage, Fetch};

use quadtree::{Quadtree, SysUpdateQuadtree};

#[derive(Clone, Debug)]
pub struct Position {
    x: f32,
    y: f32,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

struct SysUpdatePositions;

impl<'a> System<'a> for SysUpdatePositions {
    type SystemData = (WriteStorage<'a, Position>,
                       ReadStorage<'a, Vel>,
                       Fetch<'a, Quadtree>);

    fn run(&mut self, (mut pos, vel, quadtree): Self::SystemData) {
        for (pos, vel) in (&mut pos, &vel).join() {
            pos.x += vel.x;
            pos.y += vel.y;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Vel {
    x: f32,
    y: f32,
}

impl Component for Vel {
    type Storage = VecStorage<Self>;
}

fn main() {
    // Create the world
    let mut world = World::new();
    // Register components
    world.register::<Position>();
    world.register::<Vel>();
    // Add Quadtree as a resource
    world.add_resource(Quadtree::new());

    // Build dispatcher
    let mut dispatcher = DispatcherBuilder::new()
        .add(SysUpdatePositions, "update_positions", &[])
        // Make quadtree update depend on position update
        .add(SysUpdateQuadtree, "update_quadtree", &["update_positions"])
        .build();

    // Run systems
    dispatcher.dispatch(&mut world.res);
    world.maintain();
}
