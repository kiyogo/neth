# Vanilla Java 26.3 Source to Neth (Rust) Mapping

| Vanilla Java Class | Java File Path | Neth (Rust) Type / Struct | Rust File Path | Parity Status |
|---|---|---|---|---|
| `GoalSelector` | `net/minecraft/world/entity/ai/goal/GoalSelector.java` | `GoalSelector` | `crates/pumpkin-core/src/entity/ai/goal/goal_selector.rs` | Complete |
| `WrappedGoal` | `net/minecraft/world/entity/ai/goal/WrappedGoal.java` | `PrioritizedGoal` | `crates/pumpkin-core/src/entity/ai/goal/mod.rs` | Complete |
| `Goal` | `net/minecraft/world/entity/ai/goal/Goal.java` | `Goal` trait | `crates/pumpkin-core/src/entity/ai/goal/mod.rs` | Complete |
| `Goal.Flag` | `net/minecraft/world/entity/ai/goal/Goal.java` | `Controls` | `crates/pumpkin-core/src/entity/ai/goal/mod.rs` | Complete |
| `Brain` | `net/minecraft/world/entity/ai/Brain.java` | `Brain` | `crates/pumpkin-core/src/entity/ai/brain/mod.rs` | Complete |
| `Activity` | `net/minecraft/world/entity/schedule/Activity.java` | `Activity` | `crates/pumpkin-core/src/entity/ai/brain/activity.rs` | Complete |
| `MemoryModuleType` | `net/minecraft/world/entity/ai/memory/MemoryModuleType.java` | `MemoryType` | `crates/pumpkin-core/src/entity/ai/brain/memory/types.rs` | Complete |
| `ExpirableValue` | `net/minecraft/world/entity/ai/memory/ExpirableValue.java` | `MemoryValue` | `crates/pumpkin-core/src/entity/ai/brain/memory/value.rs` | Complete |
| `Sensor` | `net/minecraft/world/entity/ai/sensing/Sensor.java` | `Sensor` trait | `crates/pumpkin-core/src/entity/ai/brain/sensing/mod.rs` | Complete |
| `Sensing` | `net/minecraft/world/entity/ai/sensing/Sensing.java` | `Sensing` | `crates/pumpkin-core/src/entity/ai/sensing.rs` | Complete |
| `TargetingConditions` | `net/minecraft/world/entity/ai/targeting/TargetingConditions.java` | `TargetPredicate` | `crates/pumpkin-core/src/entity/ai/target_predicate.rs` | Complete |
| `MoveControl` | `net/minecraft/world/entity/ai/control/MoveControl.java` | `MoveControl` | `crates/pumpkin-core/src/entity/ai/control/move_control.rs` | Complete |
| `LookControl` | `net/minecraft/world/entity/ai/control/LookControl.java` | `LookControl` | `crates/pumpkin-core/src/entity/ai/control/look_control.rs` | Complete |
| `JumpControl` | `net/minecraft/world/entity/ai/control/JumpControl.java` | `JumpControl` | `crates/pumpkin-core/src/entity/ai/control/jump_control.rs` | Complete |
| `BodyRotationControl`| `net/minecraft/world/entity/ai/control/BodyRotationControl.java` | `BodyRotationControl` | `crates/pumpkin-core/src/entity/ai/control/body_rotation_control.rs` | Complete |
| `PathNavigation` | `net/minecraft/world/entity/ai/navigation/PathNavigation.java` | `Navigator` | `crates/pumpkin-core/src/entity/ai/pathfinder/mod.rs` | Complete |
| `GroundPathNavigation`| `net/minecraft/world/entity/ai/navigation/GroundPathNavigation.java` | `Navigator` (ground) | `crates/pumpkin-core/src/entity/ai/pathfinder/mod.rs` | Complete |
| `FlyingPathNavigation`| `net/minecraft/world/entity/ai/navigation/FlyingPathNavigation.java` | `Navigator` (flying) | `crates/pumpkin-core/src/entity/ai/pathfinder/mod.rs` | Complete |
| `WaterBoundPathNavigation`| `net/minecraft/world/entity/ai/navigation/WaterBoundPathNavigation.java` | `Navigator` (swim) | `crates/pumpkin-core/src/entity/ai/pathfinder/mod.rs` | Complete |
| `PathFinder` | `net/minecraft/world/level/pathfinder/PathFinder.java` | `PathFinder` | `crates/pumpkin-core/src/entity/ai/pathfinder/mod.rs` | Complete |
| `NodeEvaluator` | `net/minecraft/world/level/pathfinder/NodeEvaluator.java` | `NodeEvaluator` | `crates/pumpkin-core/src/entity/ai/pathfinder/node_evaluator.rs` | Complete |
| `WalkNodeEvaluator` | `net/minecraft/world/level/pathfinder/WalkNodeEvaluator.java` | `WalkNodeEvaluator` | `crates/pumpkin-core/src/entity/ai/pathfinder/walk_node_evaluator.rs` | Complete |
| `SwimNodeEvaluator` | `net/minecraft/world/level/pathfinder/SwimNodeEvaluator.java` | `SwimNodeEvaluator` | `crates/pumpkin-core/src/entity/ai/pathfinder/swim_node_evaluator.rs` | Complete |
| `FlyNodeEvaluator` | `net/minecraft/world/level/pathfinder/FlyNodeEvaluator.java` | `FlyNodeEvaluator` | `crates/pumpkin-core/src/entity/ai/pathfinder/fly_node_evaluator.rs` | Complete |
| `AmphibiousNodeEvaluator`| `net/minecraft/world/level/pathfinder/AmphibiousNodeEvaluator.java` | `AmphibiousNodeEvaluator` | `crates/pumpkin-core/src/entity/ai/pathfinder/amphibious_node_evaluator.rs` | Complete |
| `RandomPos` | `net/minecraft/world/entity/ai/util/RandomPos.java` | `RandomPos` | `crates/pumpkin-core/src/entity/ai/util/random_pos.rs` | Complete |
| `LandRandomPos` | `net/minecraft/world/entity/ai/util/LandRandomPos.java` | `LandRandomPos` | `crates/pumpkin-core/src/entity/ai/util/land_random_pos.rs` | Complete |
| `AirAndWaterRandomPos`| `net/minecraft/world/entity/ai/util/AirAndWaterRandomPos.java` | `AirAndWaterRandomPos` | `crates/pumpkin-core/src/entity/ai/util/air_and_water_random_pos.rs` | Complete |
| `HoverRandomPos` | `net/minecraft/world/entity/ai/util/HoverRandomPos.java` | `HoverRandomPos` | `crates/pumpkin-core/src/entity/ai/util/hover_random_pos.rs` | Complete |
| `DefaultRandomPos` | `net/minecraft/world/entity/ai/util/DefaultRandomPos.java` | `DefaultRandomPos` | `crates/pumpkin-core/src/entity/ai/util/default_random_pos.rs` | Complete |

## Goals Source Mapping

| Vanilla Goal Class | Java File Path | Neth (Rust) Goal Struct | Rust File Path | Parity Status |
|---|---|---|---|---|
| `FloatGoal` | `ai/goal/FloatGoal.java` | `SwimGoal` | `entity/ai/goal/swim.rs` | Complete |
| `PanicGoal` | `ai/goal/PanicGoal.java` | `EscapeDangerGoal` | `entity/ai/goal/escape_danger.rs` | Complete |
| `BreedGoal` | `ai/goal/BreedGoal.java` | `BreedGoal` | `entity/ai/goal/breed.rs` | Complete |
| `TemptGoal` | `ai/goal/TemptGoal.java` | `TemptGoal` | `entity/ai/goal/tempt.rs` | Complete |
| `FollowParentGoal` | `ai/goal/FollowParentGoal.java` | `FollowParentGoal` | `entity/ai/goal/follow_parent.rs` | Complete |
| `FollowOwnerGoal` | `ai/goal/FollowOwnerGoal.java` | `FollowOwnerGoal` | `entity/ai/goal/follow_owner.rs` | Complete |
| `WaterAvoidingRandomStrollGoal` | `ai/goal/WaterAvoidingRandomStrollGoal.java` | `WaterAvoidingRandomStrollGoal` | `entity/ai/goal/water_avoiding_random_stroll.rs` | Complete |
| `RandomLookAroundGoal` | `ai/goal/RandomLookAroundGoal.java` | `LookAroundGoal` | `entity/ai/goal/look_around.rs` | Complete |
| `LookAtPlayerGoal` | `ai/goal/LookAtPlayerGoal.java` | `LookAtEntityGoal` | `entity/ai/goal/look_at_entity.rs` | Complete |
| `MeleeAttackGoal` | `ai/goal/MeleeAttackGoal.java` | `MeleeAttackGoal` | `entity/ai/goal/melee_attack.rs` | Complete |
| `ZombieAttackGoal` | `ai/goal/ZombieAttackGoal.java` | `ZombieAttackGoal` | `entity/ai/goal/zombie_attack.rs` | Complete |
| `RangedBowAttackGoal` | `ai/goal/RangedBowAttackGoal.java` | `BowAttackGoal` | `entity/ai/goal/bow_attack.rs` | Complete |
| `RangedCrossbowAttackGoal` | `ai/goal/RangedCrossbowAttackGoal.java` | `RangedCrossbowAttackGoal` | `entity/ai/goal/ranged_crossbow_attack.rs` | Complete |
| `CreeperIgniteGoal` | `ai/goal/CreeperIgniteGoal.java` | `CreeperIgniteGoal` | `entity/ai/goal/creeper_ignite.rs` | Complete |
| `EatBlockGoal` | `ai/goal/EatBlockGoal.java` | `EatGrassGoal` | `entity/ai/goal/eat_grass.rs` | Complete |
| `AvoidEntityGoal` | `ai/goal/AvoidEntityGoal.java` | `AvoidEntityGoal` | `entity/ai/goal/avoid_entity.rs` | Complete |
| `LeapAtTargetGoal` | `ai/goal/LeapAtTargetGoal.java` | `LeapAtTargetGoal` | `entity/ai/goal/leap_at_target.rs` | Complete |
| `OpenDoorGoal` | `ai/goal/OpenDoorGoal.java` | `OpenDoorGoal` | `entity/ai/goal/open_door.rs` | Complete |
| `BreakDoorGoal` | `ai/goal/BreakDoorGoal.java` | `BreakDoorGoal` | `entity/ai/goal/break_door.rs` | Complete |
| `FleeSunGoal` | `ai/goal/FleeSunGoal.java` | `FleeSunGoal` | `entity/ai/goal/flee_sun.rs` | Complete |
| `RestrictSunGoal` | `ai/goal/RestrictSunGoal.java` | `RestrictSunGoal` | `entity/ai/goal/restrict_sun.rs` | Complete |
| `SitWhenOrderedToGoal` | `ai/goal/SitWhenOrderedToGoal.java` | `SitWhenOrderedToGoal` | `entity/ai/goal/sit_when_ordered_to.rs` | Complete |
| `HurtByTargetGoal` | `ai/goal/target/HurtByTargetGoal.java` | `RevengeGoal` | `entity/ai/goal/revenge.rs` | Complete |
| `NearestAttackableTargetGoal` | `ai/goal/target/NearestAttackableTargetGoal.java` | `TrackTargetGoal` | `entity/ai/goal/track_target.rs` | Complete |
| `ResetUniversalAngerTargetGoal` | `ai/goal/target/ResetUniversalAngerTargetGoal.java` | `ResetUniversalAngerGoal` | `entity/ai/goal/reset_universal_anger.rs` | Complete |
| `OwnerHurtByTargetGoal` | `ai/goal/target/OwnerHurtByTargetGoal.java` | `OwnerHurtByTargetGoal` | `entity/ai/goal/owner_hurt_by_target.rs` | Complete |
| `OwnerHurtTargetGoal` | `ai/goal/target/OwnerHurtTargetGoal.java` | `OwnerHurtTargetGoal` | `entity/ai/goal/owner_hurt_target.rs` | Complete |
| `DefendVillageTargetGoal` | `ai/goal/target/DefendVillageTargetGoal.java` | `DefendVillageGoal` | `entity/ai/goal/defend_village.rs` | Complete |
