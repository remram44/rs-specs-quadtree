extern crate specs;

mod quadtree;

use specs::{Component, DispatcherBuilder, Fetch, Join, ReadStorage,
            System, VecStorage, World, WriteStorage};

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
            let (old_x, old_y) = (pos.x, pos.y);
            pos.x += vel.x;
            pos.y += vel.y;
            println!("Move: {}, {} -> {}, {}",
                     old_x, old_y, pos.x, pos.y);
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

    // Create test entities
    world.create_entity()
        .with(Position { x: 0.0, y: 0.0 })
        .with(Vel { x: 2.0, y: 1.0 })
        .build();

    // Run systems
    dispatcher.dispatch(&mut world.res);
    world.maintain();
}
