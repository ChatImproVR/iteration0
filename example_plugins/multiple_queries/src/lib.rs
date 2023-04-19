use cimvr_engine_interface::{dbg, make_app_state, pkg_namespace, prelude::*};
use serde::{Deserialize, Serialize};

struct ClientState;

struct ServerState {
    increment: i32,
}

make_app_state!(ClientState, ServerState);

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy, Debug)]
struct MyComponent {
    a: i32,
    b: f32,
}

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy, Debug)]
struct MyOtherComponent {
    frogge: u128,
}

impl UserState for ServerState {
    fn new(io: &mut EngineIo, sched: &mut EngineSchedule<Self>) -> Self {
        io.create_entity()
            .add_component(MyComponent { a: 0, b: 0.0 })
            .add_component(MyOtherComponent { frogge: u128::MAX })
            .add_component(Synchronized)
            .build();

        sched
            .add_system(Self::update)
            .query("My Query Name")
            .intersect::<MyComponent>(Access::Write)
            .qcommit()
            .query("My Other Query Name")
            .intersect::<MyOtherComponent>(Access::Write)
            .qcommit()
            .build();

        Self { increment: 0 }
    }
}

impl ServerState {
    fn update(&mut self, _io: &mut EngineIo, query: &mut QueryResult) {
        // Update MyComponent, which is then automatically Sychronized with the connected clients
        // Note that we re-use the string "My Query Name" to refer to the query we
        for key in query.iter("My Query Name") {
            query.write(
                key,
                &MyComponent {
                    a: self.increment,
                    b: self.increment as f32,
                },
            );
        }

        for key in query.iter("My Other Query Name") {
            query.write(key, &MyOtherComponent { frogge: 0 });
        }

        self.increment += 1;
    }
}

// Client code
impl UserState for ClientState {
    fn new(_io: &mut EngineIo, sched: &mut EngineSchedule<Self>) -> Self {
        // Schedule the update() system to run every Update,
        // querying all entities with the MyComponent component attached
        sched
            .add_system(Self::update)
            .query("My query")
            .intersect::<MyComponent>(Access::Read)
            .qcommit()
            .query("My other query")
            .intersect::<MyOtherComponent>(Access::Read)
            .qcommit()
            .build();

        Self
    }
}

impl ClientState {
    fn update(&mut self, _io: &mut EngineIo, query: &mut QueryResult) {
        for key in query.iter("My query") {
            dbg!(query.read::<MyComponent>(key));
        }

        for key in query.iter("My other query") {
            dbg!(query.read::<MyOtherComponent>(key));
        }
    }
}
