use std::time::Duration;
use bevy::{
	prelude::*,
	render::camera::ScalingMode,
	core_pipeline::{
		bloom::BloomSettings,
		tonemapping::Tonemapping,
	},
	ecs::system::SystemId,
	math::bounding::{
		Aabb2d,
		BoundingVolume,
		IntersectsVolume,
	},
	sprite::{
		MaterialMesh2dBundle,
		Mesh2dHandle,
	},
};
use bevy_embedded_assets::EmbeddedAssetPlugin;
use bevy_vello::{prelude::*, VelloPlugin};

#[non_exhaustive]
struct ZLAYER;
impl ZLAYER {
	pub const FRAME: f32  = 0.0;
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

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SIZE: Vec2        = Vec2::new(10.0, 90.0);
const PADDLE_OFFSET: f32       = 300.0;
const PADDLE_SPEED: f32        = 500.0;
const PADDLE_ACCELERATION: f32 = PADDLE_SPEED * 4.0;
const PADDLE_PADDING: f32      = 10.0; // How close can the paddle get to the wall

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, 0.0, ZLAYER::BALL);
const BALL_SIZE: Vec2              = Vec2::new(10.0, 10.0);
const BALL_SPEED: f32              = 400.0;

const DEACCELERATION_DISTANCE: f32      = 50.0;
const FRAME_SIZE: Vec2                  = Vec2::new(640.0, 480.0);

const LEFT_WALL: f32   = -FRAME_SIZE.x / 2.0;
const RIGHT_WALL: f32  =  FRAME_SIZE.x / 2.0;
const BOTTOM_WALL: f32 = -FRAME_SIZE.y / 2.0;
const TOP_WALL: f32    =  FRAME_SIZE.y / 2.0;

const BACKGROUND_COLOR: Color = Color::BLACK;
const PADDLE_COLOR: Color     = Color::RED;
const BALL_COLOR: Color       = Color::RED;

const BASIC_TEXT_COLOR: Color     = Color::WHITE;
const SCORE_TEXT_COLOR: Color     = Color::DARK_GRAY;
const GAME_OVER_TEXT_COLOR: Color = Color::WHITE;

const START_DELAY: Duration     = Duration::from_secs(3);
const NEXT_SET_DELAY: Duration  = Duration::from_secs(1);
const GAME_OVER_DELAY: Duration = Duration::from_secs(3);

const TEXT_RESOLUTION: f32        = 4.0;
const GLOBAL_TEXT_SCALE: f32      = 1.0 / TEXT_RESOLUTION;
const INSTRUCTIONS_FONT_SIZE: f32 = 20.0 * TEXT_RESOLUTION;
const START_FONT_SIZE: f32        = 20.0 * TEXT_RESOLUTION;
const SCOREBOARD_FONT_SIZE: f32   = 100.0 * TEXT_RESOLUTION;
const GAME_OVER_FONT_SIZE: f32    = 60.0 * TEXT_RESOLUTION;

const WIN_CONDITIONS: u32 = 7;

// use bevy_editor_pls::prelude::*;

fn main() {
	let mut app = App::new();
	
	// Plugins
	app.add_plugins((
		DefaultPlugins,
		EmbeddedAssetPlugin::default(),
		VelloPlugin,
	));

	// States
	app.insert_state(GameplayState::Startup);
	let state_switcher = app.world.register_system(switch_to_next_state);
	app.insert_resource(NextStateSystem(state_switcher));

	// Transitions
	app.add_systems(OnEnter(GameplayState::Active), start_game_set)
		.add_systems(OnExit(GameplayState::Active), reset_game_set)
		.add_systems(OnEnter(GameplayState::GameOver), hide_ball)
		.add_systems(OnExit(GameplayState::GameOver), (reset_scoreboard, unhide_ball));

	// Events
	app.add_event::<CollisionEvent>();

	// Resources
	app.insert_resource(Scoreboard { score_left: 0, score_right: 0 })
		.insert_resource(ClearColor(BACKGROUND_COLOR))
		.insert_resource(StateTimer(Timer::default()));

	// Systems: startup
	app.add_systems(Startup, world_setup);

	// System: update
	app.add_systems(FixedUpdate,
		(
		player_control,
		bound_paddle,
		apply_velocity,
			(
			check_ball_collisions,
			on_collision_play_sound,
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
		update_text_with_scoreboard.run_if(in_state(GameplayState::Active)),
		tick_timer                 .run_if(in_state(GameplayState::NextSet)),
		wait_for_response          .run_if(in_state(GameplayState::GameOver)),
		));

	// Systems: temporarely
	app.add_systems(Update, bevy::window::close_on_esc);

	app.run();
}

// Components
#[derive(Component)] struct Paddle;
#[derive(Component)] struct Ball;
#[derive(Component, Deref, DerefMut)] struct Velocity(Vec2);
#[derive(Component)] struct Collider;
#[derive(Component)] struct ScoreboardUi;
#[derive(Component)] struct AdaptiveResolution;
#[derive(Component)] struct Player;
#[derive(Component)] struct Ai;
#[derive(Component)] struct Paragraph { when_visible: GameplayState }

// Events
#[derive(Event, Default)] struct CollisionEvent;

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
            velocity: Velocity(Vec2::ZERO),
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
#[derive(Resource)] struct NextStateSystem(SystemId);
#[derive(Resource)] struct StateTimer(Timer);
#[derive(Resource)] struct Scoreboard { score_left: u32, score_right: u32 }
#[derive(Resource)] struct CollisionSound(Handle<AudioSource>);

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
				scaling_mode: ScalingMode::AutoMin { min_width: FRAME_SIZE.x, min_height: FRAME_SIZE.y},
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
	let ball_collision_sound = asset_server.load("embedded://sounds/ball_collision.ogg");
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

	// Paragraphs
	let font_nums   = asset_server.load("embedded://fonts/basicallyamono-bold.otf");
	let font_bold   = asset_server.load("embedded://fonts/sundaymasthead.otf");
	let font_medium = asset_server.load("embedded://fonts/openinghourssans.otf");
	commands.spawn(ParagraphBundle::new(
		GameplayState::Instructions,
		Vec2::new(0.0, -160.0),
		Text::from_section("Use 'Up' and 'Down' arrows to move paddle\nEnter space to start game...",
			TextStyle {
				font: font_medium.clone(),
				font_size: INSTRUCTIONS_FONT_SIZE,
				color: BASIC_TEXT_COLOR })
			.with_justify(JustifyText::Center),
		));
	commands.spawn(ParagraphBundle::new(
		GameplayState::Start,
		Vec2::new(0.0, -160.0),
		Text::from_section("Be ready!", TextStyle {
			font: font_medium,
			font_size: START_FONT_SIZE,
			color: BASIC_TEXT_COLOR })
			.with_justify(JustifyText::Center),
		));
	commands.spawn((
		ScoreboardUi,
		ParagraphBundle::new(
			GameplayState::Active,
			Vec2::new(0.0, 0.0),
			Text::from_section("", TextStyle {
				font: font_nums,
				font_size: SCOREBOARD_FONT_SIZE,
				color: SCORE_TEXT_COLOR })
				.with_justify(JustifyText::Center),
		)));
	commands.spawn(ParagraphBundle::new(
		GameplayState::GameOver,
		Vec2::new(-7.0, 0.0),
		Text::from_section("GAME OVER", TextStyle {
			font: font_bold,
			font_size: GAME_OVER_FONT_SIZE,
			color: GAME_OVER_TEXT_COLOR })
			.with_justify(JustifyText::Center),
		));

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

fn on_collision_play_sound(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    sound: Res<CollisionSound>,
) {
    // Play a sound once per frame if a collision occurred.
    if !collision_events.is_empty() {
        collision_events.clear();
        commands.spawn(AudioBundle {
            source: sound.0.clone(),
            settings: PlaybackSettings::DESPAWN,
        });
    }
}

fn check_win_conditions(scoreboard: Res<Scoreboard>) -> GameplayState {
	match scoreboard.score_left >= WIN_CONDITIONS || scoreboard.score_right >= WIN_CONDITIONS	{
		true  => GameplayState::GameOver,
		false => GameplayState::NextSet,
	}
	// let (mut ball_velocity, mut ball_visibility) = ball_query.single_mut();
	// *ball_visibility = Visibility::Hidden;
	// ball_velocity.0 = Vec2::ZERO;
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
		GameplayState::GameOver => reset_timer(timer, GAME_OVER_DELAY),
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
	timer.0.set_duration(duration);
	timer.0.reset();
}

fn tick_timer(
	time: Res<Time>,
	state_switcher: Res<NextStateSystem>,
	mut timer: ResMut<StateTimer>,
	mut commands: Commands,
) {
	timer.0.tick(time.delta());
	if timer.0.just_finished()
	{
		commands.run_system(state_switcher.0);
	}
}

fn wait_for_response(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	state_switcher: Res<NextStateSystem>,
	mut commands: Commands,
) {
	if keyboard_input.pressed(KeyCode::Space)
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
	
	ball_velocity.0 = Vec2::new(SIN_OF_45, SIN_OF_45) * BALL_SPEED;
}

fn reset_game_set(
	mut ball_query: Query<(&mut Velocity, &mut Transform), With<Ball>>,
) {
	let (mut ball_velocity, mut ball_transform) = ball_query.single_mut();
	
	ball_velocity.0 = Vec2::ZERO;
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