use bevy::{
	core_pipeline::{bloom::{BloomCompositeMode, BloomSettings}, tonemapping::Tonemapping, }, ecs::query, math::{bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume}, vec2}, prelude::*, render::camera::ScalingMode, scene::ron::to_string, sprite::{MaterialMesh2dBundle, Mesh2dHandle}, transform, window::WindowResized
};
use bevy_editor_pls::prelude::*;

#[non_exhaustive]
struct ZLAYER;
impl ZLAYER {
	pub const SPACE: f32 = 0.0;
    pub const SCOREBOARD: f32 = 1.0;
    pub const MAIN: f32 = 2.0;
    pub const BALL: f32 = 3.0;
	pub const CAMERA: f32 = 4.0;
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum CollisionH { Left, Right }
enum CollisionV { Top, Bottom }

const SIN_OF_45: f32 = 0.70710678118654752440084436210485;

// #[inline]
// #[must_use]
// fn from_angle(angle: f32) -> Self {
// 	let (sin, cos) = math::sin_cos(angle);
// 	Self { x: cos, y: sin }
// }

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SIZE: Vec2 = Vec2::new(10.0, 90.0);
const PADDLE_OFFSET: f32 = 300.0;
const PADDLE_SPEED: f32 = 500.0;
const PADDLE_ACCELERATION: f32 = PADDLE_SPEED * 4.0;
const PADDLE_PADDING: f32 = 10.0; // How close can the paddle get to the wall

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, 0.0, ZLAYER::BALL);
const BALL_SIZE: Vec2 = Vec2::new(10.0, 10.0);
const BALL_SPEED: f32 = 400.0;
const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(SIN_OF_45, SIN_OF_45);

const DEACCELERATION_DISTANCE: f32 = 50.0;
const SPACE_SIZE: Vec2 = Vec2::new(640.0, 480.0);
const SCOREBOARD_RESOLUTION_SCALE: f32 = 4.0;
const SCOREBOARD_FONT_SIZE: f32 = 40.0 * SCOREBOARD_RESOLUTION_SCALE;
const SCOREBOARD_SCALE: f32 = 1.0 / SCOREBOARD_RESOLUTION_SCALE;

const LEFT_WALL: f32 = -SPACE_SIZE.x / 2.0;   // x
const RIGHT_WALL: f32 = SPACE_SIZE.x / 2.0;   // x
const BOTTOM_WALL: f32 = -SPACE_SIZE.y / 2.0; // y
const TOP_WALL: f32 = SPACE_SIZE.y / 2.0;     // y

const BACKGROUND_COLOR: Color = Color::BLACK;
const SPACE_COLOR: Color = Color::DARK_GRAY;
const PADDLE_COLOR: Color = Color::RED;
const BALL_COLOR: Color = Color::RED;
const TITLE_COLOR: Color = Color::YELLOW;
const SCORE_COLOR: Color = Color::GRAY;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins)
		// .add_plugins(EditorPlugin::default()) // for debugging
		.insert_resource(Scoreboard { score_left: 0, score_right: 0 })
		.insert_resource(ClearColor(BACKGROUND_COLOR))
		.add_event::<CollisionEvent>()
		.add_systems(Startup, setup)
		.add_systems(FixedUpdate, (
			player_control,
			bound_paddle,
			apply_velocity,
			check_ball_collisions,
			play_collision_sound,
		).chain(),)
		.add_systems(Update, (update_text_with_scoreboard, bevy::window::close_on_esc))
		.run();
}

// Components
#[derive(Component)] struct Paddle;
#[derive(Component)] struct Ball;
#[derive(Component, Deref, DerefMut)] struct Velocity(Vec2);
#[derive(Component)] struct Collider;
#[derive(Event, Default)] struct CollisionEvent;
#[derive(Component)] struct ScoreboardUi;
#[derive(Component)] struct AdaptiveResolution;
#[derive(Component)] struct Player;
#[derive(Component)] struct Ai;
//#[derive(Debug, PartialEq, Eq, Copy, Clone)]

// Bundles
#[derive(Bundle)] struct PaddleBundle {
    paddle: Paddle,
	collider: Collider,
    velocity: Velocity,
}

impl PaddleBundle {
    fn new() -> Self {
        Self {
            paddle: Paddle,
			collider: Collider,
            velocity: Velocity(Vec2::ZERO),
        }
    }
}

#[derive(Bundle)] struct BallBundle {
    ball: Ball,
    velocity: Velocity,
}

impl BallBundle {
    fn new() -> Self {
        Self {
            ball: Ball,
            velocity: Velocity(INITIAL_BALL_DIRECTION * BALL_SPEED),
        }
    }
}

// Resources
#[derive(Resource)] struct Scoreboard { score_left: usize, score_right: usize }
#[derive(Resource)] struct MainCamera { id: Entity }
#[derive(Resource)] struct TextScale { scale: f32 }
#[derive(Resource)] struct CollisionSound(Handle<AudioSource>);

fn setup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<ColorMaterial>>,
	asset_server: Res<AssetServer>,
) {
	// Camera
	commands.spawn((
		Camera2dBundle {
			projection: OrthographicProjection {
				scaling_mode: ScalingMode::AutoMin { min_width: SPACE_SIZE.x, min_height: SPACE_SIZE.y},
				..default()
			},
			camera: Camera {
				hdr: true, // HDR is required for bloom
				..default()
			},
			tonemapping: Tonemapping::TonyMcMapface, // tonemapper that desaturates to white
			transform: Transform::from_xyz(0.0, 0.0, ZLAYER::CAMERA),
			..default()
		},
		BloomSettings::default(), // Enable bloom for the camera
	));

	// Sound
	let ball_collision_sound = asset_server.load("sounds/ball_collision.ogg");
	commands.insert_resource(CollisionSound(ball_collision_sound));

	// Ball
	commands.spawn((
		BallBundle::new(),
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(meshes.add(Rectangle::from_size(BALL_SIZE))),
			material: materials.add(BALL_COLOR),
			transform: Transform::from_translation(BALL_STARTING_POSITION),
			..default()
		},
	));

	// Paddles
	let paddle_mesh = meshes.add(Rectangle::from_size(PADDLE_SIZE));
	let paddle_material = materials.add(PADDLE_COLOR);
	commands.spawn((
		PaddleBundle::new(),
		Player,
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(paddle_mesh.clone()),
			material: paddle_material.clone(),
			transform: Transform::from_xyz(PADDLE_OFFSET, 0.0, ZLAYER::MAIN),
			..default()
		},
	));
	commands.spawn((
		PaddleBundle::new(),
		Ai,
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(paddle_mesh),
			material: paddle_material,
			transform: Transform::from_xyz(-PADDLE_OFFSET, 0.0, ZLAYER::MAIN),
			..default()
		},
	));

	// Scoreboard
	let font = asset_server.load("fonts/FiraSans-Bold.ttf");
	let text_justification = JustifyText::Center;
	let text_style = TextStyle { font: font.clone(), font_size: SCOREBOARD_FONT_SIZE, color: SCORE_COLOR };
	commands.spawn((
		ScoreboardUi,
		Text2dBundle {
			text: Text::from_section("0 : 0", text_style).with_justify(text_justification),
			transform: Transform::from_xyz(0.0, 0.0, ZLAYER::SCOREBOARD).with_scale(Vec3::splat(SCOREBOARD_SCALE)),
			..default()
		},
	));

	// Background
	commands.spawn((
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(meshes.add(Rectangle::from_size(SPACE_SIZE))),
			material: materials.add(SPACE_COLOR),
			transform: Transform::from_xyz(0.0, 0.0, ZLAYER::SPACE),
			..default()
		},
	));
}

fn apply_velocity(
	mut query: Query<(&mut Transform, &Velocity)>,
	time: Res<Time>
) {
	for (mut transform, velocity) in &mut query {
		transform.translation.x += velocity.x * time.delta_seconds();
		transform.translation.y += velocity.y * time.delta_seconds();
	}
}

fn bound_paddle(
	mut query: Query<(&Transform, &mut Velocity), With<Paddle>>,
) {
	for (paddle_transform, mut paddle_velocity) in &mut query
	{
		let paddle_y = paddle_transform.translation.y;
	
		let bottom_bound = BOTTOM_WALL + PADDLE_SIZE.y / 2.0 + PADDLE_PADDING;
		let top_bound    = TOP_WALL    - PADDLE_SIZE.y / 2.0 - PADDLE_PADDING;
	
		let max_velocity_at_top    = ((top_bound    - paddle_y) / DEACCELERATION_DISTANCE) * PADDLE_SPEED;
		let min_velocity_at_bottom = ((bottom_bound - paddle_y) / DEACCELERATION_DISTANCE) * PADDLE_SPEED;
		paddle_velocity.y = paddle_velocity.y.clamp(min_velocity_at_bottom, max_velocity_at_top);
	}
}

fn player_control(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut query: Query<&mut Velocity, (With<Paddle>, With<Player>)>,
	time: Res<Time>,
) {
	let mut velocity = query.single_mut();
	let mut direction_y = 0.0;

	if keyboard_input.pressed(KeyCode::ArrowUp)   { direction_y += 1.0; }
	if keyboard_input.pressed(KeyCode::ArrowDown) { direction_y -= 1.0; }

	let max_diff = PADDLE_ACCELERATION * time.delta_seconds();
	let velocity_goal_y = direction_y * PADDLE_SPEED;
	let velocity_diff_y = velocity_goal_y - velocity.y;
	let delta_velocity_y = velocity_diff_y.clamp(-max_diff, max_diff);

	velocity.y += delta_velocity_y;
}

fn update_text_with_scoreboard(
	scoreboard: Res<Scoreboard>,
	mut query: Query<&mut Text, With<ScoreboardUi>>,
) {
	let mut text = query.single_mut();
	text.sections[0].value = format!("{} : {}",
		scoreboard.score_left.to_string(),
		scoreboard.score_right.to_string(),
	);
}

fn check_ball_collisions(
    mut scoreboard: ResMut<Scoreboard>,
    mut ball_query: Query<(&mut Velocity, &Transform), With<Ball>>,
    collider_query: Query<&Transform, With<Collider>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
	let (mut ball_velocity, ball_transform) = ball_query.single_mut();
	let mut maybe_collision = collide_with_walls(Aabb2d::new(ball_transform.translation.xy(), BALL_SIZE / 2.0));

	for transform in &collider_query
	{
		let (collision_h, collision_v) = collide_with_collider(
			Aabb2d::new(ball_transform.translation.xy(), BALL_SIZE / 2.0),
			Aabb2d::new(transform.translation.xy(), PADDLE_SIZE * transform.scale.xy() / 2.0),
		);

		if collision_h.is_some() { maybe_collision.0 = collision_h; }
		if collision_v.is_some() { maybe_collision.1 = collision_v; }
	}

	let mut collision_detected = false;
	
	if let Some(collision_h) = maybe_collision.0 {
		collision_detected = true;
		let reflect_x;
		match collision_h {
			CollisionH::Left  => reflect_x = ball_velocity.x < 0.0,
			CollisionH::Right => reflect_x = ball_velocity.x > 0.0,
		}
		if reflect_x { ball_velocity.x = -ball_velocity.x; }
	}

	if let Some(collision_v) = maybe_collision.1 {
		collision_detected = true;
		let reflect_y;
		match collision_v {
			CollisionV::Top    => reflect_y = ball_velocity.y > 0.0,
			CollisionV::Bottom => reflect_y = ball_velocity.y < 0.0,
		}
		if reflect_y { ball_velocity.y = -ball_velocity.y; }
	}

	if collision_detected {
		collision_events.send_default();
	}
}

fn collide_with_walls(ball: Aabb2d) -> (Option<CollisionH>, Option<CollisionV>)
{
	let mut side = (None, None);
    if (ball.center().x - ball.half_size().x) <= LEFT_WALL { side.0 = Some(CollisionH::Left); }
	if (ball.center().x + ball.half_size().x) >= RIGHT_WALL { side.0 = Some(CollisionH::Right); }
	
	if (ball.center().y - ball.half_size().y) <= BOTTOM_WALL { side.1 = Some(CollisionV::Bottom); }
	if (ball.center().y + ball.half_size().y) >= TOP_WALL { side.1 = Some(CollisionV::Top); }

	side
}

fn collide_with_collider(ball: Aabb2d, collider: Aabb2d) -> (Option<CollisionH>, Option<CollisionV>)
{
	if !ball.intersects(&collider) {
		return (None, None);
	}

	let closest = collider.closest_point(ball.center());
    let offset = ball.center() - closest; // offset of the ball relative to the closest point
    let side = if offset.x.abs() > offset.y.abs() {
        if offset.x < 0. {
			(Some(CollisionH::Right), None)
		} else {
            (Some(CollisionH::Left), None)
        }
    } else if offset.y > 0. {
		(None, Some(CollisionV::Bottom))
	} else {
        (None, Some(CollisionV::Top))
    };

	side
}

fn play_collision_sound(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    sound: Res<CollisionSound>,
) {
    // Play a sound once per frame if a collision occurred.
    if !collision_events.is_empty() {
        // This prevents events staying active on the next frame.
        collision_events.clear();
        commands.spawn(AudioBundle {
            source: sound.0.clone(),
            // auto-despawn the entity when playback finishes
            settings: PlaybackSettings::DESPAWN,
        });
    }
}