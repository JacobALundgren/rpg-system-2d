use bevy::prelude::*;
use bevy::sprite::collide_aabb::collide;

use crate::player::Player;

pub struct AreaPlugin;

#[derive(Clone, Component, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AreaIdentifier(pub usize);

impl From<usize> for AreaIdentifier {
    fn from(val: usize) -> Self {
        AreaIdentifier(val)
    }
}

#[derive(Resource)]
pub struct GameAreas {
    areas: Vec<Area>,
}

impl GameAreas {
    pub fn new(areas: Vec<Area>) -> Self {
        Self { areas }
    }
}

#[derive(Clone, Copy, Debug, Event, PartialEq)]
struct AreaTransitionEvent(PassageDestination);

fn area_startup_system(
    mut commands: Commands,
    mut background: ResMut<ClearColor>,
    game_areas: Res<GameAreas>,
) {
    game_areas.areas[0].load(&mut commands, &mut background);
}

impl Plugin for AreaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::rgb(1., 0., 0.)))
            .add_event::<AreaTransitionEvent>()
            .add_systems(Startup, area_startup_system)
            .add_systems(Update, area_transition_check)
            .add_systems(Update, area_transition)
            .add_systems(Update, area_transition_drawing);
    }
}

#[derive(Clone, Debug)]
pub struct Passage {
    transform: Transform,
    sprite: Sprite,
    destination: AreaIdentifier,
    destination_transform: Transform,
}

impl Passage {
    pub fn new(
        transform: Transform,
        sprite: Sprite,
        destination: AreaIdentifier,
        destination_transform: Transform,
    ) -> Self {
        Passage {
            transform,
            sprite,
            destination,
            destination_transform,
        }
    }
}

impl PartialEq for Passage {
    fn eq(&self, other: &Self) -> bool {
        self.transform == other.transform
            && self.sprite.custom_size == other.sprite.custom_size
            && self.destination == other.destination
            && self.destination_transform == other.destination_transform
    }
}

#[derive(Clone, Component, Copy, Debug, PartialEq)]
struct PassageDestination(AreaIdentifier, Transform);

#[derive(Clone)]
pub struct Area {
    color: Color,
    passages: Vec<Passage>,
}

impl Area {
    pub fn new(color: Color, passages: Vec<Passage>) -> Self {
        Area { color, passages }
    }

    fn load(&self, commands: &mut Commands, background: &mut ResMut<ClearColor>) {
        background.0 = self.color;
        for passage in &self.passages {
            commands
                .spawn(SpriteBundle {
                    transform: passage.transform,
                    sprite: passage.sprite.clone(),
                    ..Default::default()
                })
                .insert(PassageDestination(
                    passage.destination,
                    passage.destination_transform,
                ));
        }
    }
}

fn area_transition_check(
    player_query: Query<(&Player, &Transform, &Sprite)>,
    passages_query: Query<(&PassageDestination, &Transform, &Sprite)>,
    mut ev_area_transition: EventWriter<AreaTransitionEvent>,
) {
    let player = player_query.get_single().unwrap();
    if let Some((destination, _, _)) =
        passages_query
            .iter()
            .by_ref()
            .find(|&(_, transform, sprite)| {
                collide(
                    player.1.translation,
                    player.2.custom_size.unwrap(),
                    transform.translation,
                    sprite.custom_size.unwrap(),
                )
                .is_some()
            })
    {
        ev_area_transition.send(AreaTransitionEvent(PassageDestination(
            destination.0,
            destination.1,
        )));
    }
}

fn area_transition(
    mut commands: Commands,
    mut player_query: Query<(&Player, &mut Transform)>,
    mut ev_area_transition: EventReader<AreaTransitionEvent>,
    game_areas: Res<GameAreas>,
    mut background: ResMut<ClearColor>,
    passages: Query<(Entity, &PassageDestination)>,
) {
    if let Some(destination) = ev_area_transition.read().next() {
        assert!(destination.0 .0 .0 < game_areas.areas.len());
        for passage in passages.iter() {
            commands.entity(passage.0).despawn();
        }
        if let Ok((_, mut transform)) = player_query.get_single_mut() {
            transform.translation = destination.0 .1.translation;
        }
        game_areas.areas[destination.0 .0 .0].load(&mut commands, &mut background);
    }
}

fn area_transition_drawing(
    mut ev_area_transition: EventReader<AreaTransitionEvent>,
    mut drawable_query: Query<(&AreaIdentifier, &mut Visibility)>,
) {
    if let Some(destination) = ev_area_transition.read().next() {
        for (&area, ref mut visibility) in drawable_query.iter_mut() {
            let entered_area = destination.0 .0;
            if area == entered_area {
                **visibility = Visibility::Visible;
            } else {
                **visibility = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Area, AreaIdentifier, AreaTransitionEvent, GameAreas, Passage, PassageDestination,
    };
    use crate::enemy::Enemy;
    use crate::player::Player;
    use bevy::prelude::{
        App, ClearColor, Color, Events, IntoSystem, MinimalPlugins, Schedule, Sprite, Transform,
        Vec2, Visibility, World,
    };
    use bevy::utils::default;

    fn run_system<S: IntoSystem<(), (), P>, P: Send + Sync + 'static>(
        world: &mut World,
        system: S,
    ) {
        let mut schedule = Schedule::default();
        schedule.add_systems(system);
        schedule.run(world);
    }

    fn get_test_app() -> App {
        let mut ret = App::default();
        ret.add_plugins(MinimalPlugins);
        ret.add_plugins(bevy::asset::AssetPlugin::default());
        ret
    }

    #[test]
    fn transitions_are_detected() {
        let mut app = get_test_app();
        let mut world = &mut app.world;
        world
            .spawn(Player::default())
            .insert(Transform::from_xyz(10., 10., 1.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2.1, 2.1)),
                ..default()
            });
        world
            .spawn(PassageDestination(
                1.into(),
                Transform::from_xyz(20., 20., 2.),
            ))
            .insert(Transform::from_xyz(12., 12., 100.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            });
        world.insert_resource(Events::<AreaTransitionEvent>::default());
        run_system(&mut world, super::area_transition_check);
        let area_transition_events = world.get_resource::<Events<AreaTransitionEvent>>().unwrap();
        let mut reader = area_transition_events.get_reader();
        let mut iter = reader.read(&area_transition_events);
        assert_eq!(
            *iter.next().unwrap(),
            AreaTransitionEvent(PassageDestination(
                1.into(),
                Transform::from_xyz(20., 20., 2.)
            ))
        );
        assert!(iter.next().is_none());
    }

    #[test]
    fn multiple_collisions_send_one_event() {
        let mut app = get_test_app();
        let mut world = &mut app.world;
        world
            .spawn(Player::default())
            .insert(Transform::from_xyz(10., 10., 1.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2.1, 2.1)),
                ..default()
            });
        let destination1 = PassageDestination(1.into(), Transform::from_xyz(20., 20., 2.));
        world
            .spawn(destination1)
            .insert(Transform::from_xyz(12., 12., 100.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            });
        let destination2 = PassageDestination(2.into(), Transform::from_xyz(25., 25., 10.));
        world
            .spawn(destination2)
            .insert(Transform::from_xyz(8., 8., 100.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            });
        world.insert_resource(Events::<AreaTransitionEvent>::default());
        run_system(&mut world, super::area_transition_check);
        let area_transition_events = world.get_resource::<Events<AreaTransitionEvent>>().unwrap();
        let mut reader = area_transition_events.get_reader();
        let mut iter = reader.read(&area_transition_events);
        let received_destination = iter.next().unwrap().0;
        assert!(received_destination == destination1 || received_destination == destination2);
        assert!(iter.next().is_none());
    }

    fn get_test_areas() -> GameAreas {
        let passage_out1 = Passage {
            transform: Transform::from_xyz(12., 12., 100.),
            sprite: Sprite {
                color: Color::rgb(0., 1., 0.),
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            },
            destination: 1.into(),
            destination_transform: Transform::from_xyz(20., 20., 2.),
        };
        let passage_out2 = Passage {
            transform: Transform::from_xyz(92., 92., 100.),
            sprite: Sprite {
                color: Color::rgb(0., 1., 0.),
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            },
            destination: 1.into(),
            destination_transform: Transform::from_xyz(50., 50., 5.),
        };
        let passage_in1 = Passage {
            transform: Transform::from_xyz(60., 60., 100.),
            sprite: Sprite {
                color: Color::rgb(0., 1., 0.),
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            },
            destination: 0.into(),
            destination_transform: Transform::from_xyz(40., 40., 8.),
        };
        let passage_in2 = Passage {
            transform: Transform::from_xyz(160., 160., 100.),
            sprite: Sprite {
                color: Color::rgb(0., 1., 0.),
                custom_size: Some(Vec2::new(2., 2.)),
                ..default()
            },
            destination: 0.into(),
            destination_transform: Transform::from_xyz(140., 140., 18.),
        };
        GameAreas {
            areas: [
                Area {
                    color: Color::rgb(0.125, 0.82, 0.325),
                    passages: vec![passage_out1, passage_out2],
                },
                Area {
                    color: Color::rgb(0.251, 0.521, 0.382),
                    passages: vec![passage_in1, passage_in2],
                },
            ]
            .into(),
        }
    }

    // This really shouldn't need the World as mutable, but I can't find a good way to query
    // without it
    fn check_area_is_loaded(world: &mut World, area: &Area) {
        assert_eq!(area.color, world.get_resource::<ClearColor>().unwrap().0);
        let mut expected_passages = area.passages.clone();
        let mut passage_query = world.query::<(&PassageDestination, &Transform, &Sprite)>();
        for (dest, transform, sprite) in passage_query.iter(&world) {
            let pos = expected_passages.iter().position(|passage| {
                *passage
                    == Passage {
                        transform: *transform,
                        sprite: sprite.clone(),
                        destination: dest.0,
                        destination_transform: dest.1,
                    }
            });
            assert!(pos.is_some());
            expected_passages.remove(pos.unwrap());
        }
        assert!(expected_passages.is_empty());
    }

    #[test]
    fn area_transition() {
        let mut app = get_test_app();
        let mut world = &mut app.world;
        world.insert_resource(get_test_areas());
        world
            .spawn(Player::default())
            .insert(Transform::from_xyz(10., 10., 1.))
            .insert(Sprite {
                custom_size: Some(Vec2::new(2.1, 2.1)),
                ..default()
            });
        world.insert_resource(Events::<AreaTransitionEvent>::default());
        world.insert_resource(ClearColor::default());
        run_system(&mut world, super::area_startup_system);
        run_system(&mut world, super::area_transition_check);
        run_system(&mut world, super::area_transition);
        let player = {
            let mut player_query = world.query::<(&Player, &Transform)>();
            let mut player_query_iter = player_query.iter(&world);
            let player = player_query_iter.next().unwrap();
            assert!(player_query_iter.next().is_none());
            player
        };
        let areas = &world.get_resource::<GameAreas>().unwrap().areas.clone();
        assert_eq!(*player.1, areas[0].passages[0].destination_transform);
        check_area_is_loaded(&mut world, &areas[1]);
    }

    #[test]
    fn area_transition_drawing() {
        let mut app = get_test_app();
        let mut world = &mut app.world;
        world.insert_resource(get_test_areas());
        world
            .spawn(Enemy::default())
            .insert(Visibility::Hidden)
            .insert(Transform::from_xyz(40., 40., 1.))
            .insert(AreaIdentifier(1));
        world.insert_resource(Events::<AreaTransitionEvent>::default());
        let mut area_transition_events = world
            .get_resource_mut::<Events<AreaTransitionEvent>>()
            .unwrap();
        area_transition_events.send(AreaTransitionEvent(PassageDestination(
            1.into(),
            Transform::default(),
        )));
        run_system(&mut world, super::area_transition_drawing);
        let visibility = {
            let mut enemy_query = world.query::<(&Enemy, &Visibility)>();
            let mut enemy_query_iter = enemy_query.iter(&world);
            let enemy = enemy_query_iter.next().unwrap();
            assert!(enemy_query_iter.next().is_none());
            enemy.1
        };
        assert_eq!(visibility, Visibility::Visible);
    }
}
