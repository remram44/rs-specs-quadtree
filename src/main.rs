extern crate specs;

mod quadtree;

use specs::{Component, DispatcherBuilder, Fetch, Join, ReadStorage,
            System, VecStorage, World, WriteStorage};

use quadtree::{Bounds, Quadtree, QuadtreeRef, SysUpdateQuadtree};

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
                       WriteStorage<'a, Bounds>,
                       ReadStorage<'a, Vel>,
                       Fetch<'a, Quadtree>);

    fn run(&mut self, (mut pos, mut bounds, vel, quadtree): Self::SystemData) {
        for (pos, bounds, vel) in (&mut pos, &mut bounds, &vel).join() {
            let (old_x, old_y) = (pos.x, pos.y);
            pos.x += vel.x;
            pos.y += vel.y;
            println!("Move: {}, {} -> {}, {}",
                     old_x, old_y, pos.x, pos.y);

            let half_size = bounds.size * 0.5;
            bounds.pos.x = pos.x - half_size;
            bounds.pos.y = pos.y - half_size;
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
    world.register::<Bounds>();
    world.register::<QuadtreeRef>();
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
        .with(Bounds {
            pos: Position { x: -0.3, y: -0.3 },
            size: 0.6
        })
        .build();

    // Run systems
    dispatcher.dispatch(&mut world.res);
    world.maintain();
}
