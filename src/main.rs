// import std
use std::time::Duration;

// import bevy
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_pipeline::bloom::{BloomSettings, BloomPrefilterSettings, BloomCompositeMode};
use bevy::ecs::system::SystemId;
use bevy::math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume, };
use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};
use bevy::window::{PresentMode, WindowMode, WindowTheme};
use bevy::app::AppExit;
use bevy::audio::Volume;

// import custom
use bevy_embedded_assets::EmbeddedAssetPlugin;
use bevy_vello::{prelude::*, VelloPlugin};

#[non_exhaustive]
struct ZLAYER;
impl ZLAYER {
	pub const FRAME: f32  = 0.0;
	pub const SCORE: f32  = 0.5;
	pub const TEXT: f32   = 1.0;
	pub const MAIN: f32   = 2.0;
	pub const BALL: f32   = 3.0;
	pub const CAMERA: f32 = 4.0;
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)] enum CollisionH { Left, Right }
#[derive(Debug, PartialEq, Eq, Copy, Clone)] enum CollisionV { Top, Bottom }
#[derive(States, Debug, Clone, PartialEq, Eq, Hash)] enum GameplayState {
	Startup,
	Instructions,
	Start,
	Active,
	NextSet,
	GameOver,
}

const SIN_OF_45: f32 = 0.70710678118654752440084436210485;

const PADDLE_SIZE: Vec2     = Vec2::new(10.0, 90.0);
const PADDLE_OFFSET_X: f32  = 300.0;

const PLAYER_ACCELERATION: f32   = 2000.0;
const PLAYER_MAX_SPEED: f32      = 500.0;
const AI_STARTING_MAX_SPEED: f32 = 500.0;

const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, 0.0, ZLAYER::BALL);
const BALL_SIZE: Vec2              = Vec2::new(10.0, 10.0);
const BALL_STARTING_SPEED: f32     = 400.0;
const BALL_DELTA_SPEED: f32        = 10.0;

const FRAME_SIZE: Vec2 = Vec2::new(640.0, 480.0);
const LEFT_WALL: f32   = -FRAME_SIZE.x / 2.0;
const RIGHT_WALL: f32  =  FRAME_SIZE.x / 2.0;
const BOTTOM_WALL: f32 = -FRAME_SIZE.y / 2.0 + WALL_THICKNESS;
const TOP_WALL: f32    =  FRAME_SIZE.y / 2.0 - WALL_THICKNESS;

const WALL_THICKNESS: f32         = 6.0;

const RED_COLOR: Color            = Color::rgb(2.0, 0.0, 0.0);
const GOLD_COLOR: Color           = Color::rgb(2.0, 1.68, 0.0);

const BACKGROUND_COLOR: Color     = Color::BLACK;
const PADDLE_COLOR: Color         = RED_COLOR;
const BALL_COLOR: Color           = RED_COLOR;
const BASIC_TEXT_COLOR: Color     = Color::WHITE;
const SCORE_TEXT_COLOR: Color     = Color::DARK_GRAY;
const VICTORY_TEXT_COLOR: Color   = GOLD_COLOR;
const DEFEAT_TEXT_COLOR: Color    = RED_COLOR;

const START_DELAY: Duration     = Duration::from_secs(3);
const NEXT_SET_DELAY: Duration  = Duration::from_secs(1);
const HOLD_TO_EXIT: Duration    = Duration::from_secs(2);

const TEXT_RESOLUTION: f32        = 4.0;
const GLOBAL_TEXT_SCALE: f32      = 1.0 / TEXT_RESOLUTION;
const INSTRUCTIONS_FONT_SIZE: f32 = TEXT_RESOLUTION * 20.0;
const INSTR_ICONS_FONT_SIZE: f32  = TEXT_RESOLUTION * 23.0;
const START_FONT_SIZE: f32        = TEXT_RESOLUTION * 20.0;
const SCORE_FONT_SIZE: f32        = TEXT_RESOLUTION * 300.0;
const GAME_OVER_FONT_SIZE: f32    = TEXT_RESOLUTION * 60.0;

const WIN_CONDITIONS: u32 = 3;

const PROJECTION_WIDTH: f32  = FRAME_SIZE.x + 40.0;
const PROJECTION_HEIGHT: f32 = FRAME_SIZE.y + 40.0;

const KEYCODES_ACCEPT: [KeyCode; 2]       = [KeyCode::Space, KeyCode::Enter];
const KEYCODES_PADDLE_RIGHT: [KeyCode; 4] = [KeyCode::ArrowUp,  KeyCode::ArrowRight, KeyCode::KeyW, KeyCode::KeyD];
const KEYCODES_PADDLE_LEFT: [KeyCode; 4]  = [KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::KeyS, KeyCode::KeyA];
const KEYCODE_EXIT: KeyCode               = KeyCode::Escape;
const KEYCODE_FULLSCREEN: KeyCode         = KeyCode::F11;
const KEYCODE_VOLUME_UP: KeyCode          = KeyCode::F10;
const KEYCODE_VOLUME_DOWN: KeyCode        = KeyCode::F9;

fn main() {
	let mut app = App::new();
	
	// Plugins
	app.add_plugins((
		DefaultPlugins.set(WindowPlugin {
			primary_window: Some(Window {
				resolution: FRAME_SIZE.into(),
				present_mode: PresentMode::AutoVsync,
				window_theme: Some(WindowTheme::Dark),
				mode: WindowMode::Fullscreen,
				..default()
				}),
			..default()
			}),
		EmbeddedAssetPlugin::default(),
		VelloPlugin,
	));

	// States
	app.insert_state(GameplayState::Startup);
	let state_switcher = app.world.register_system(switch_to_next_state);
	app.insert_resource(NextStateSystem(state_switcher));

	// Transitions
	app.add_systems(OnExit(GameplayState::Instructions), (
		unhide_ball,
		unhide_scoreboard,
		))
		.add_systems(OnEnter(GameplayState::Active), start_game_set)
		.add_systems(OnExit(GameplayState::Active), (reset_game_set, update_text_with_scoreboard))
		.add_systems(OnEnter(GameplayState::GameOver), (
			hide_ball,
			hide_scoreboard,
			update_game_over,
		))
		.add_systems(OnExit(GameplayState::GameOver), (
			(reset_scoreboard, update_text_with_scoreboard).chain(),
			unhide_ball,
			unhide_scoreboard,
		));

	// Events
	app.add_event::<CollisionEvent>();

	// Resources
	app.insert_resource(Scoreboard { score_left: 0, score_right: 0 })
		.insert_resource(ClearColor(BACKGROUND_COLOR))
		.insert_resource(GlobalVolume(Volume::default()))
		.insert_resource(ExitTimer(Timer::new(HOLD_TO_EXIT, TimerMode::Once)))
		.insert_resource(StateTimer(Timer::default()));

	// Systems: startup
	app.add_systems(Startup, world_setup);

	// System: window
	app.add_systems(Update, toggle_window_mode);

	// System: update
	app.add_systems(Update,
		(
		(
		player_control,
		ai_control,
		),
		limit_velocity,
		apply_velocity,
		bound_paddle,
			(
			check_ball_collisions,
			on_collision_actions,
			)
			.chain()
			.run_if(in_state(GameplayState::Active)),
		)
		.chain()
		.run_if(not(in_state(GameplayState::Startup))
		));

	// Systems: for each GameplayState
	app.add_systems(Update,
		(
		wait_for_response          .run_if(in_state(GameplayState::Instructions)),
		tick_timer                 .run_if(in_state(GameplayState::Start)),
		tick_timer                 .run_if(in_state(GameplayState::NextSet)),
		wait_for_response          .run_if(in_state(GameplayState::GameOver)),
		));

	// Systems: other
	app.add_systems(Update, (exit_on_esc, volume_control));

	app.run();
}

// Components
#[derive(Component)] struct Paddle;
#[derive(Component)] struct Ball;
#[derive(Component, Deref, DerefMut)] struct Velocity(Vec2);
#[derive(Component, Deref, DerefMut)] struct MaxSpeed(f32);
#[derive(Component)] struct Collider;
#[derive(Component)] struct ScoreboardUi;
#[derive(Component)] struct GameOverUi;
#[derive(Component)] struct ExitUi;
#[derive(Component)] struct AdaptiveResolution;
#[derive(Component)] struct Player;
#[derive(Component)] struct Ai;
#[derive(Component, Deref, DerefMut)] struct Paragraph { when_visible: GameplayState }

// Events
#[derive(Event, Default)] struct CollisionEvent;

// Bundles
#[derive(Bundle)] struct PaddleBundle {
	paddle: Paddle,
	collider: Collider,
	velocity: Velocity,
	max_speed: MaxSpeed,
}
impl PaddleBundle {
	fn new(max_speed: f32) -> Self {
		Self {
			paddle: Paddle,
			collider: Collider,
			velocity: Velocity(Vec2::ZERO),
			max_speed: MaxSpeed(max_speed),
		}
	}
}

#[derive(Bundle)] struct BallBundle {
	ball: Ball,
	velocity: Velocity,
	max_speed: MaxSpeed,
}
impl BallBundle {
	fn new() -> Self {
		Self {
			ball: Ball,
			velocity: Velocity(Vec2::ZERO),
			max_speed: MaxSpeed(BALL_STARTING_SPEED),
		}
	}
}

#[derive(Bundle)] struct ParagraphBundle {
	information: Paragraph,
	text_bundle: Text2dBundle,
}
impl ParagraphBundle {
	fn new(state: GameplayState, position: Vec2, text: Text) -> Self {
		Self {
			information: Paragraph { when_visible: state },
			text_bundle: Text2dBundle {
				text: text,
				visibility: Visibility::Hidden,
				transform: Transform::from_xyz(position.x, position.y, ZLAYER::TEXT)
				.with_scale(Vec3::splat(GLOBAL_TEXT_SCALE)),
				..default()
			},
		}
	}
}

// Resources
#[derive(Resource, Deref, DerefMut)] struct NextStateSystem(SystemId);
#[derive(Resource, Deref, DerefMut)] struct StateTimer(Timer);
#[derive(Resource, Deref, DerefMut)] struct ExitTimer(Timer);
#[derive(Resource)] struct Scoreboard { score_left: u32, score_right: u32 }
#[derive(Resource, Deref, DerefMut)] struct CollisionSound(Handle<AudioSource>);
#[derive(Resource, Deref, DerefMut)] struct GlobalVolume(Volume);

fn world_setup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<ColorMaterial>>,
	asset_server: Res<AssetServer>,
	state_switcher: Res<NextStateSystem>,
) {
	// Camera
	commands.spawn((
		Camera2dBundle {
			projection: OrthographicProjection {
				scaling_mode: ScalingMode::AutoMin { min_width: PROJECTION_WIDTH, min_height: PROJECTION_HEIGHT},
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
		BloomSettings {
			intensity: 0.15,
			low_frequency_boost: 0.7,
			low_frequency_boost_curvature: 0.95,
			high_pass_frequency: 1.0,
			prefilter_settings: BloomPrefilterSettings {
				threshold: 0.6,
				threshold_softness: 0.2,
			},
			composite_mode: BloomCompositeMode::Additive,
		},
	));

	// Sound
	let ball_collision_sound = asset_server.load("embedded://sounds/ball_collision.ogg");
	commands.insert_resource(CollisionSound(ball_collision_sound));

	// Ball
	commands.spawn((
		BallBundle::new(),
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(meshes.add(Rectangle::from_size(BALL_SIZE))),
			material: materials.add(BALL_COLOR),
			transform: Transform::from_translation(BALL_STARTING_POSITION),
			visibility: Visibility::Hidden,
			..default()
		},
	));

	// Paddles
	let paddle_mesh = meshes.add(Rectangle::from_size(PADDLE_SIZE));
	let paddle_material = materials.add(PADDLE_COLOR);
	commands.spawn((
		PaddleBundle::new(PLAYER_MAX_SPEED),
		Player,
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(paddle_mesh.clone()),
			material: paddle_material.clone(),
			transform: Transform::from_xyz(PADDLE_OFFSET_X, 0.0, ZLAYER::MAIN),
			..default()
		},
	));
	commands.spawn((
		PaddleBundle::new(AI_STARTING_MAX_SPEED),
		Ai,
		MaterialMesh2dBundle {
			mesh: Mesh2dHandle(paddle_mesh),
			material: paddle_material,
			transform: Transform::from_xyz(-PADDLE_OFFSET_X, 0.0, ZLAYER::MAIN),
			..default()
		},
	));

	// Paragraphs
	let font_icons: Handle<Font> = asset_server.load("embedded://fonts/promptfont.otf");
	let font_bold   = asset_server.load("embedded://fonts/sundaymasthead.otf");
	let font_medium = asset_server.load("embedded://fonts/openinghourssans.otf");
	commands.spawn(ParagraphBundle::new(
		GameplayState::Instructions,
		Vec2::new(-80.0, 0.0),
		Text::from_section("Movement\nSound\nFullscreen\nExit\n\nAccept",
			TextStyle {
				font: font_medium.clone(),
				font_size: INSTRUCTIONS_FONT_SIZE,
				color: BASIC_TEXT_COLOR })
				.with_justify(JustifyText::Right),
		));
	commands.spawn(ParagraphBundle::new(
		GameplayState::Instructions,
		Vec2::new(50.0, 0.0),
		Text::from_section("⏶⏷\n⑨⑩\n⑪\n␯\n\n␮",
			TextStyle {
				font: font_icons,
				font_size: INSTR_ICONS_FONT_SIZE,
				color: BASIC_TEXT_COLOR })
				.with_justify(JustifyText::Right),
		));
	commands.spawn(ParagraphBundle::new(
		GameplayState::Start,
		Vec2::new(0.0, -160.0),
		Text::from_section("Be ready!", TextStyle {
			font: font_medium.clone(),
			font_size: START_FONT_SIZE,
			color: BASIC_TEXT_COLOR }),
		));
	commands.spawn((
		GameOverUi,
		ParagraphBundle::new(
			GameplayState::GameOver,
			Vec2::new(0.0, 0.0),
			Text::from_section("", TextStyle {
				font: font_bold,
				font_size: GAME_OVER_FONT_SIZE,
				color: BASIC_TEXT_COLOR }),
		)));

	// Scoreboard
	commands.spawn((
		ScoreboardUi,
		Text2dBundle {
			text:
				Text::from_section("0 0", TextStyle {
				font: asset_server.load("embedded://fonts/basicallyamono-bold.otf"),
				font_size: SCORE_FONT_SIZE,
				color: SCORE_TEXT_COLOR }),
			transform:
				Transform::from_xyz(0.0, 0.0, ZLAYER::SCORE)
				.with_scale(Vec3::splat(GLOBAL_TEXT_SCALE)),
			visibility:
				Visibility::Hidden,
			..default()
		}));

	// Exit UI
	commands.spawn((
		ExitUi,
		Text2dBundle {
			text:
				Text::from_section("Holding ESC to exit", TextStyle {
				font: font_medium,
				font_size: INSTRUCTIONS_FONT_SIZE,
				color: BASIC_TEXT_COLOR }),
			transform:
				Transform::from_xyz(-210.0, 205.0, ZLAYER::TEXT)
				.with_scale(Vec3::splat(GLOBAL_TEXT_SCALE)),
			visibility:
				Visibility::Hidden,
			..default()
		}));

	// Frame
	commands.spawn(VelloAssetBundle {
		vector: asset_server.load("embedded://textures/frame.svg"),
		debug_visualizations: DebugVisualizations::Hidden,
		transform: Transform::from_xyz(0.0, 0.0, ZLAYER::FRAME).with_scale(Vec3::splat(1.0)),
		..default()
	});

	// Start game
	commands.run_system(state_switcher.0);
}

fn player_control(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut query: Query<&mut Velocity, (With<Paddle>, With<Player>)>,
	time: Res<Time>,
) {
	let mut velocity = query.single_mut();
	
	let is_up   = keyboard_input.any_pressed(KEYCODES_PADDLE_RIGHT);
	let is_down = keyboard_input.any_pressed(KEYCODES_PADDLE_LEFT);
	let direction_y = f32::from(is_up) - f32::from(is_down);

	let max_delta_vel_y  = PLAYER_ACCELERATION * time.delta_seconds();
	let velocity_goal_y  = direction_y * PLAYER_MAX_SPEED;
	let delta_velocity_y = velocity_goal_y - velocity.y;

	velocity.y += delta_velocity_y.clamp(-max_delta_vel_y, max_delta_vel_y);
}

fn ai_control(
	mut paddle_query: Query<(&Transform, &mut Velocity), (With<Paddle>, With<Ai>)>,
	ball_query: Query<&Transform, With<Ball>>,
	time: Res<Time>,
) {
	if time.delta_seconds() == 0.0 { return }

	let (transform, mut velocity) = paddle_query.single_mut();
	let ball_transform = ball_query.single();
	
	let delta_distance = ball_transform.translation.y - transform.translation.y;
	velocity.y = delta_distance / time.delta_seconds();
}

fn limit_velocity(
	mut query: Query<(&mut Velocity, &MaxSpeed)>,
) {
	for (mut velocity, max_speed) in &mut query
	{
		velocity.0 = velocity.clamp_length_max(max_speed.0);
	}
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
	mut query: Query<(&mut Transform, &mut Velocity), With<Paddle>>,
) {
	for (mut transform, mut velocity) in &mut query
	{
		const BOUND: f32 = TOP_WALL - PADDLE_SIZE.y / 2.0;
		let translation_goal_y = transform.translation.y.clamp(-BOUND, BOUND);
		
		if transform.translation.y == translation_goal_y { continue }

		transform.translation.y = translation_goal_y;
		velocity.0.y = 0.0;
	}
}

fn update_text_with_scoreboard(
	scoreboard: Res<Scoreboard>,
	mut query: Query<&mut Text, With<ScoreboardUi>>,
) {
	let mut binding = query.single_mut(); // panic
 	let text_section = binding.sections.first_mut().unwrap(); // panic
	
	text_section.value = format!("{} {}",
		scoreboard.score_left.to_string(),
		scoreboard.score_right.to_string(),
	);
}

fn check_ball_collisions(
	mut commands: Commands,
	state_switcher: Res<NextStateSystem>,
	mut scoreboard: ResMut<Scoreboard>,
	mut ball_query: Query<(&mut Velocity, &Transform), With<Ball>>,
	collider_query: Query<&Transform, With<Collider>>,
	mut collision_events: EventWriter<CollisionEvent>,
) {
	let (mut ball_velocity, ball_transform) = ball_query.single_mut();

	// collide with walls
	let mut maybe_collision = collide_with_walls(Aabb2d::new(ball_transform.translation.xy(), BALL_SIZE / 2.0));

	if maybe_collision.0.is_some() { commands.run_system(state_switcher.0) }

	// process scoreboard
	match maybe_collision.0 {
		Some(CollisionH::Left)  => scoreboard.score_right += 1,
		Some(CollisionH::Right) => scoreboard.score_left += 1,
		None => ()
	}

	// collide with colliders
	for transform in &collider_query
	{
		let (collision_h, collision_v) = collide_with_collider(
			Aabb2d::new(ball_transform.translation.xy(), BALL_SIZE / 2.0),
			Aabb2d::new(transform.translation.xy(), PADDLE_SIZE * transform.scale.xy() / 2.0),
		);

		if collision_h.is_some() { maybe_collision.0 = collision_h; }
		if collision_v.is_some() { maybe_collision.1 = collision_v; }
	}

	// change velocity
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

	// collision event
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

fn on_collision_actions(
	mut commands: Commands,
	mut collision_events: EventReader<CollisionEvent>,
	mut query: Query<(&mut Velocity, &mut MaxSpeed), With<Ball>>,
	sound: Res<CollisionSound>,
	volume: Res<GlobalVolume>,
) {
	// Play a sound once per frame if a collision occurred.
	if collision_events.is_empty() { return }
	
	// Play sound
	commands.spawn(AudioBundle {
		source: sound.clone(),
		settings: PlaybackSettings::DESPAWN.with_volume(volume.0),
	});

	// Increase speed
	let (mut velocity, mut max_speed) = query.single_mut();
	max_speed.0 += BALL_DELTA_SPEED;
	velocity.0 = velocity.clamp_length_min(max_speed.0);

	collision_events.clear();
}

fn check_win_conditions(scoreboard: Res<Scoreboard>) -> GameplayState {
	match scoreboard.score_left >= WIN_CONDITIONS || scoreboard.score_right >= WIN_CONDITIONS	{
		true  => GameplayState::GameOver,
		false => GameplayState::NextSet,
	}
}

fn switch_to_next_state(
	scoreboard: Res<Scoreboard>,
	current_game_state: Res<State<GameplayState>>,
	mut next_game_state: ResMut<NextState<GameplayState>>,
	mut paragraph_query: Query<(&mut Visibility, &Paragraph)>,
	timer: ResMut<StateTimer>,
) {
	let state = match current_game_state.get() {
		GameplayState::Startup      => GameplayState::Instructions,
		GameplayState::Instructions => GameplayState::Start,
		GameplayState::Start        => GameplayState::Active,
		GameplayState::Active       => check_win_conditions(scoreboard),
		GameplayState::NextSet      => GameplayState::Active,
		GameplayState::GameOver     => GameplayState::Start,
	};

	match state {
		GameplayState::Start    => reset_timer(timer, START_DELAY),
		GameplayState::NextSet  => reset_timer(timer, NEXT_SET_DELAY),
		_                       => (),
	};

	// Set visibility for paragraphs
	for (mut p_visibility, paragraph) in &mut paragraph_query {
		*p_visibility = match paragraph.when_visible == state {
			true  => Visibility::Inherited,
			false => Visibility::Hidden,
		};
	}

	next_game_state.set(state);
}

fn reset_timer(
	mut timer: ResMut<StateTimer>,
	duration: Duration,
) {
	timer.set_duration(duration);
	timer.reset();
}

fn tick_timer(
	time: Res<Time>,
	state_switcher: Res<NextStateSystem>,
	mut timer: ResMut<StateTimer>,
	mut commands: Commands,
) {
	timer.tick(time.delta());
	if timer.just_finished()
	{
		commands.run_system(state_switcher.0);
	}
}

fn wait_for_response(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	state_switcher: Res<NextStateSystem>,
	mut commands: Commands,
) {
	if keyboard_input.any_just_pressed(KEYCODES_ACCEPT)
	{
		commands.run_system(state_switcher.0);
	}
}

fn reset_scoreboard(
	mut scoreboard: ResMut<Scoreboard>,
) {
	scoreboard.score_left  = 0;
	scoreboard.score_right = 0;
}

fn start_game_set(
	mut ball_query: Query<&mut Velocity, With<Ball>>,
) {
	let mut ball_velocity = ball_query.single_mut();
	
	ball_velocity.0 = Vec2::new(SIN_OF_45, SIN_OF_45) * BALL_STARTING_SPEED;
}

fn reset_game_set(
	mut ball_query: Query<(&mut Velocity, &mut MaxSpeed, &mut Transform), With<Ball>>,
) {
	let (mut ball_velocity, mut max_speed, mut ball_transform) = ball_query.single_mut();
	
	ball_velocity.0 = Vec2::ZERO;
	max_speed.0 = BALL_STARTING_SPEED;
	ball_transform.translation = BALL_STARTING_POSITION;
}

fn hide_ball(
	mut ball_query: Query<&mut Visibility, With<Ball>>,
) {
	let mut ball_visibility = ball_query.single_mut();
	*ball_visibility = Visibility::Hidden;
}

fn unhide_ball(
	mut ball_query: Query<&mut Visibility, With<Ball>>,
) {
	let mut ball_visibility = ball_query.single_mut();
	*ball_visibility = Visibility::Inherited;
}

fn hide_scoreboard(
	mut query: Query<&mut Visibility, With<ScoreboardUi>>,
) {
	let mut visibility = query.single_mut();
	*visibility = Visibility::Hidden;
}

fn unhide_scoreboard(
	mut query: Query<&mut Visibility, With<ScoreboardUi>>,
) {
	let mut visibility = query.single_mut();
	*visibility = Visibility::Inherited;
}

fn update_game_over(
	scoreboard: Res<Scoreboard>,
	mut query: Query<&mut Text, With<GameOverUi>>
) {
	let mut text = query.single_mut();
	let section = text.sections.first_mut().unwrap();
	
	if scoreboard.score_right >= WIN_CONDITIONS {
		section.style.color = VICTORY_TEXT_COLOR;
		section.value = "VICTORY".into();
	} else {
		section.style.color = DEFEAT_TEXT_COLOR;
		section.value = "DEFEAT".into();
	}
	
}

fn toggle_window_mode(
	input: Res<ButtonInput<KeyCode>>,
	mut windows: Query<&mut Window>
) {
	if input.just_pressed(KEYCODE_FULLSCREEN) {
		let mut window = windows.single_mut();

		window.mode = if matches!(window.mode, WindowMode::Fullscreen) {
			WindowMode::Windowed
		} else {
			WindowMode::Fullscreen
		};

		info!("WINDOW_MODE: {:?}", window.mode);
	}
}

fn exit_on_esc(
	mut exit: EventWriter<AppExit>,
	mut exit_ui: Query<(&mut Visibility, &mut Text), With<ExitUi>>,
	mut timer: ResMut<ExitTimer>,
	
	input: Res<ButtonInput<KeyCode>>,
	time: Res<Time>,
) {
	let (mut visibility, mut text) = exit_ui.single_mut();
	let color = &mut text.sections.first_mut().unwrap().style.color;
	
	if input.just_released(KEYCODE_EXIT)
	{
		*visibility = Visibility::Hidden;
		timer.reset();
		return;
	}
	if input.just_pressed(KEYCODE_EXIT)
	{
		*visibility = Visibility::Inherited;
		color.set_a(0.0);
	}
	if input.pressed(KEYCODE_EXIT)
	{
		let bezier = CubicSegment::new_bezier((0.85, 0.06), (0.34, 0.69));
		color.set_a(bezier.ease(timer.fraction()) * 3.0);
		
		timer.tick(time.delta());
		if timer.just_finished() { exit.send(AppExit); }
	}
}

fn volume_control(
    input: Res<ButtonInput<KeyCode>>,
	mut volume: ResMut<GlobalVolume>,
) {
	let mut delta_volume = 0.0;
    
	if input.just_pressed(KEYCODE_VOLUME_UP)   { delta_volume =  0.1 }
	if input.just_pressed(KEYCODE_VOLUME_DOWN) { delta_volume = -0.1 }
	if delta_volume == 0.0 { return }
	
	let set_volume = (volume.get() + delta_volume).clamp(0.0, 1.0);
	volume.0 = Volume::new(set_volume);
}
