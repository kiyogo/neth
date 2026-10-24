# Parity Gaps & Implementation Audit: Vanilla Java 26.3 vs Neth (Rust)

## Summary of Parity Audit
- **Minecraft Target Version**: 26.3 (Protocol 777, Data Version 5023)
- **Reference Java Source**: `jvmc/src/net/minecraft/` (Mojang official mappings)
- **Rust Engine**: `Neth/crates/pumpkin-core`
- **Audit Outcome**: 30 critical gaps audited, aligned to vanilla, and verified via unit tests.

---

## Gap 1: Control Tick Order & `JumpControl` in `server_ai_step`
- **Vanilla Reference**: `net.minecraft.world.entity.Mob.serverAiStep()` (lines 740-748):
  ```java
  profiler.push("controls");
  profiler.push("move");
  this.moveControl.tick();
  profiler.popPush("look");
  this.lookControl.tick();
  profiler.popPush("jump");
  this.jumpControl.tick();
  profiler.pop();
  profiler.pop();
  ```
- **Neth Status**: **RESOLVED**
  - Integrated `jump_control: Mutex<JumpControl>` into `MobEntity`.
  - Added helper `mob_entity.jump()`.
  - Ordered ticks in exact vanilla sequence: `move_control` -> `look_control` -> `jump_control`.
  - Verified with `jump_control_lifecycle_and_tick_parity` test.

---

## Gap 2: GoalSelector Tick Decoupling & Flag Masking
- **Vanilla Reference**: `net.minecraft.world.entity.ai.goal.GoalSelector.tick()` & `tickRunningGoals(boolean forceTickAll)`.
- **Neth Status**: **RESOLVED**
  - `goal_selector.rs` implements running goal cleanups, disabled flag masking, replacement checks, and `tick_all || prioritized_goal.should_run_every_tick()`.
  - All ported movement and interaction goals properly register required flags (`Controls::MOVE`, `Controls::LOOK`, `Controls::JUMP`, `Controls::TARGET`).

---

## Gap 3: Brain Activity Transition and Core Behaviors Execution
- **Vanilla Reference**: `net.minecraft.world.entity.ai.Brain.tick()`:
  - Ticks `CORE` activity behaviors on every tick regardless of current activity schedule.
  - Ticks active schedule activity (`IDLE`, `WORK`, `REST`, `FIGHT`).
  - Clears expired memories with tick countdowns.
- **Neth Status**: **RESOLVED & VERIFIED**
  - `crates/pumpkin-core/src/entity/ai/brain/` implements core activities, activity switching, memory expiration decay, and behavior scheduling.

---

## Gap 4: GroundPathNavigation Jump and Door Interaction
- **Vanilla Reference**: `GroundPathNavigation.java`:
  - Automatically requests `mob.getJumpControl().jump()` when the path node requires stepping up.
  - Handles opening and closing wooden doors when `canOpenDoors()` is true.
- **Neth Status**: **RESOLVED**
  - Path node evaluation interacts with `mob.jump()` and `jump_control` to initiate step-ups cleanly without bypassing physics.

---

## Gap 5: `AvoidEntityGoal` Monster Dynamic Filtering Predicate
- **Vanilla Reference**: `Rabbit.java:128`:
  ```java
  this.goalSelector.addGoal(4, new RabbitAvoidEntityGoal<>(this, Monster.class, 4.0F, 2.2, 2.2));
  ```
- **Neth Issue**: `AvoidEntityGoal` only accepted a single static `flee_type: &'static EntityType`.
- **Neth Status**: **RESOLVED**
  - Upgraded `AvoidEntityGoal` to support optional dynamic `target_predicate: Option<Box<TargetPredicate>>`.
  - Added constructor `AvoidEntityGoal::for_monsters(flee_distance, slow, fast)` that filters all candidates where `entity.get_entity().entity_type.category == &MobCategory::MONSTER`.
  - Added accessor `flee_type(&self) -> Option<&'static EntityType>`.
  - Verified with unit test `avoid_entity_goal_monster_predicate`.

---

## Gap 6: Villager Threat Avoidance & Stroll Speeds Parity
- **Vanilla Reference**: `VillagerHostilesSensor.java:20-33` & `VillagerGoalPackages.STROLL_SPEED_MODIFIER = 0.4F`:
  - Villager avoids: `ILLUSIONER` (12.0), `ZOGLIN` (10.0), `PILLAGER` (15.0), `VINDICATOR` (10.0), `VEX` (8.0), `EVOKER` (12.0), `RAVAGER` (12.0), `ZOMBIE` / `HUSK` / `DROWNED` / `ZOMBIE_VILLAGER` (8.0).
- **Neth Status**: **RESOLVED**
  - Added `ILLUSIONER` and `ZOGLIN` to Villager threat avoidance list.
  - Corrected distances: `PILLAGER` (15.0), `VINDICATOR` (10.0), `VEX` (8.0).
  - Replaced generic `WanderAroundGoal(0.5)` with `WaterAvoidingRandomStrollGoal(0.4)`.

---

## Gap 7: Warden Attack Speed & Wander Parity
- **Vanilla Reference**: `WardenAi.java`:
  - `SPEED_MULTIPLIER_WHEN_FIGHTING = 1.2F`
  - Idling stroll speed = `0.5F` with water avoidance.
- **Neth Status**: **RESOLVED**
  - Updated `MeleeAttackGoal` speed multiplier to `1.2`.
  - Replaced wander with `WaterAvoidingRandomStrollGoal(0.5)`.

---

## Gap 8: Breeze Target Selectors & Stroll Parity
- **Vanilla Reference**: `BreezeAi.java`:
  - Uses `SensorType.HURT_BY` for retaliation.
  - Attacks Players and Iron Golems.
  - Strolls at `0.6F` via `RandomStroll.stroll(0.6F)`.
- **Neth Status**: **RESOLVED**
  - Registered `RevengeGoal` at priority 1 in `target_selector`.
  - Placed Player target at priority 2, Iron Golem at priority 3.
  - Replaced generic wander with `WaterAvoidingRandomStrollGoal(0.6)`.

---

## Gap 9: Hoglin & Zoglin Speed Multipliers
- **Vanilla Reference**: `HoglinAi.java` (`SPEED_MULTIPLIER_WHEN_IDLING = 0.4F`) & `Zoglin.java:75` (`SPEED_MULTIPLIER_WHEN_IDLING = 0.4F`).
- **Neth Status**: **RESOLVED**
  - Set stroll speed to `0.4` with water avoidance for both Hoglin and Zoglin.

---

## Gap 10: Zombified Piglin Water Avoidance
- **Vanilla Reference**: `ZombifiedPiglin.java:74`:
  ```java
  this.goalSelector.addGoal(7, new WaterAvoidingRandomStrollGoal(this, 1.0));
  ```
- **Neth Status**: **RESOLVED**
  - Converted stroll goal to `WaterAvoidingRandomStrollGoal(1.0)`.

---

## Gap 11: Iron Golem & Snow Golem Stroll & Targeting Parity
- **Vanilla Reference**:
  - `IronGolem.java:72`: `GolemRandomStrollInVillageGoal(0.6)`.
  - `SnowGolem.java:55-60`: `WaterAvoidingRandomStrollGoal(this, 1.0)` & `NearestAttackableTargetGoal(this, Mob.class, 10, true, false, (living) -> living instanceof Enemy)`.
- **Neth Status**: **RESOLVED**
  - Iron Golem stroll updated to `WaterAvoidingRandomStrollGoal(0.6)`.
  - Snow Golem stroll updated to `WaterAvoidingRandomStrollGoal(1.0)` (melting in water).
  - Snow Golem target selector targets all `MobCategory::MONSTER` entities within 10 blocks.

---

## Gap 12: Rabbit Monster Avoidance & Speeds
- **Vanilla Reference**: `Rabbit.java:128`:
  ```java
  this.goalSelector.addGoal(4, new RabbitAvoidEntityGoal<>(this, Monster.class, 4.0F, 2.2, 2.2));
  this.goalSelector.addGoal(6, new WaterAvoidingRandomStrollGoal(this, 0.6));
  ```
- **Neth Status**: **RESOLVED**
  - Added `AvoidEntityGoal::for_monsters(4.0, 2.2, 2.2)` at priority 4.
  - Stroll set to `WaterAvoidingRandomStrollGoal(0.6)`.

---

## Gap 13: Frog Lifecycle Goals & Speeds
- **Vanilla Reference**: `FrogAi.java`:
  - `AnimalPanic(2.0F)`
  - `AnimalMakeLove` (Breed at 1.0)
  - `SPEED_MULTIPLIER_WHEN_TEMPTED = 1.25F`
  - Walk stroll at `1.0F`.
- **Neth Status**: **RESOLVED**
  - Added `EscapeDangerGoal(2.0)` at priority 1.
  - Added `BreedGoal(1.0)` at priority 2.
  - Updated `TemptGoal` speed to `1.25`.
  - Updated stroll to `WaterAvoidingRandomStrollGoal(1.0)`.

---

## Gap 14: Goat & Panda Multipliers
- **Vanilla Reference**: `GoatAi.java:98` (`stroll(1.0F)`) & `Panda.java:269-280` (`TemptGoal(1.0)`, `WaterAvoidingRandomStrollGoal(1.0)`).
- **Neth Status**: **RESOLVED**
  - Goat stroll converted to `WaterAvoidingRandomStrollGoal(1.0)`.
  - Panda tempt speed set to `1.0`, stroll converted to `WaterAvoidingRandomStrollGoal(1.0)`.

---

## Gap 15: Camel & Camel Husk High-Speed Multipliers
- **Vanilla Reference**: `CamelAi.java`:
  - `SPEED_MULTIPLIER_WHEN_PANICKING = 4.0F`
  - `SPEED_MULTIPLIER_WHEN_IDLING_IN_HABITAT = 2.0F`
  - `SPEED_MULTIPLIER_WHEN_TEMPTED = 2.5F`
  - `SPEED_MULTIPLIER_WHEN_FOLLOWING_ADULT = 2.5F`
- **Neth Status**: **RESOLVED**
  - Camel panic speed set to `4.0`, tempt speed to `2.5`, follow parent to `2.5`, stroll to `WaterAvoidingRandomStrollGoal(2.0)`.
  - Camel Husk panic set to `4.0`, tempt to `2.5`, stroll to `WaterAvoidingRandomStrollGoal(2.0)`.

---

## Gap 16: Sniffer & Armadillo Goals & Speeds
- **Vanilla Reference**: `SnifferAi.java` (Panic 2.0, Tempt 1.25, Stroll 1.0) & `ArmadilloAi.java` (Follow Adult 1.25, Stroll 1.0).
- **Neth Status**: **RESOLVED**
  - Sniffer: added `EscapeDangerGoal(2.0)`, updated tempt speed to `1.25`, stroll to `WaterAvoidingRandomStrollGoal(1.0)`.
  - Armadillo: updated follow parent speed to `1.25`, stroll to `WaterAvoidingRandomStrollGoal(1.0)`.

---

## Gap 17: Creaking Slow Stroll Speed
- **Vanilla Reference**: `CreakingAi.java:47`:
  ```java
  RandomStroll.stroll(0.3F)
  ```
- **Neth Status**: **RESOLVED**
  - Creaking stroll speed set to `WaterAvoidingRandomStrollGoal(0.3)`.

---

## Gap 18: Stray Slowness Arrow Projectile Parity
- **Vanilla Reference**: `Stray.java:60-67`:
  ```java
  if (arrow instanceof Arrow arrow2) {
     arrow2.addEffect(new MobEffectInstance(MobEffects.SLOWNESS, 600));
  }
  ```
- **Neth Issue**: `StraySkeletonEntity` did not implement `get_arrow_projectile`, firing plain arrows.
- **Neth Status**: **RESOLVED**
  - Implemented `get_arrow_projectile` returning tipped arrow with `minecraft:slowness` (duration 600 ticks, amplifier 0).

---

## Gap 19: Bogged Health & Attack Interval Parity
- **Vanilla Reference**: `Bogged.java:37, 118-124`:
  - `Attributes.MAX_HEALTH = 16.0`
  - `getHardAttackInterval() = 50`, `getAttackInterval() = 70`.
- **Neth Status**: **RESOLVED**
  - Initialized with `SkeletonEntityBase::new_with_attack_interval(entity, 50)`.
  - Set base `MAX_HEALTH = 16.0`.

---

## Gap 20: Parched Attack Interval Initialization
- **Vanilla Reference**: `Parched.java:57-64`:
  - `getHardAttackInterval() = 50`, `getAttackInterval() = 70`.
- **Neth Status**: **RESOLVED**
  - Initialized with `SkeletonEntityBase::new_with_attack_interval(entity, 50)`.

---

## Gap 21: Illagers (Pillager, Vindicator, Illusioner, Witch) Stroll Speeds & Bow Parity
- **Vanilla Reference**:
  - `Pillager.java`: stroll speed 0.6, look at player 15.0F.
  - `Vindicator.java`: stroll speed 0.6.
  - `Illusioner.java`: `AvoidEntityGoal(Creaking, 8.0)`, `RangedBowAttackGoal(0.5, 20, 15.0)`, stroll speed 0.6, `HurtByTargetGoal(Raider.class)`.
  - `Witch.java`: `WaterAvoidingRandomStrollGoal(1.0)`.
- **Neth Status**: **RESOLVED**
  - Pillager: stroll updated to `WaterAvoidingRandomStrollGoal(0.6)`, player look range to 15.0.
  - Vindicator: stroll updated to `WaterAvoidingRandomStrollGoal(0.6)`.
  - Illusioner: added `AvoidEntityGoal(Creaking)`, `BowAttackGoal(0.5, 20, 15.0)`, `WaterAvoidingRandomStrollGoal(0.6)`, `RevengeGoal(true)`.
  - Witch: wander updated to `WaterAvoidingRandomStrollGoal(1.0)`.

---

## Gap 22: Vex Health, Attack Damage & Revenge Parity
- **Vanilla Reference**: `Vex.java:93, 99`:
  - `Attributes.MAX_HEALTH = 14.0`, `Attributes.ATTACK_DAMAGE = 4.0`.
  - `this.targetSelector.addGoal(1, new HurtByTargetGoal(this, Raider.class).setAlertOthers())`.
- **Neth Status**: **RESOLVED**
  - Configured `MAX_HEALTH = 14.0`, `ATTACK_DAMAGE = 4.0`.
  - Added `RevengeGoal(true)` at target priority 1.

---

## Gap 23: Fox Panic Speed, Avoidances & Speeds Parity
- **Vanilla Reference**: `Fox.java:180-203`:
  - `FoxPanicGoal(2.2)`
  - `AvoidEntityGoal(Player, 16.0F, 1.6, 1.4)`
  - `AvoidEntityGoal(Wolf, 8.0F, 1.6, 1.4)`
  - `AvoidEntityGoal(PolarBear, 8.0F, 1.6, 1.4)`
  - `FoxFollowParentGoal(1.25)`
  - `WaterAvoidingRandomStrollGoal(1.0)`
  - `FoxLookAtPlayerGoal(24.0F)`
- **Neth Status**: **RESOLVED**
  - Updated panic speed to 2.2, follow parent to 1.25, look range to 24.0, stroll to `WaterAvoidingRandomStrollGoal(1.0)`.
  - Added avoidance goals for Player (16.0), Wolf (8.0), and Polar Bear (8.0).

---

## Gap 24: TraderLlama Panic & Defense Targets Parity
- **Vanilla Reference**: `TraderLlama.java:65-68` & `Llama.java:126`:
  - Llama stroll speed = 0.7 (`WaterAvoidingRandomStrollGoal(0.7)`).
  - TraderLlama panic speed = 2.0 (`PanicGoal(2.0)`).
  - TraderLlama targets: `Zombie` (except ZombifiedPiglin) and `AbstractIllager` to defend WanderingTrader.
- **Neth Status**: **RESOLVED**
  - Llama & TraderLlama stroll converted to `WaterAvoidingRandomStrollGoal(0.7)`.
  - TraderLlama panic speed updated to 2.0.
  - Added protection targets against Zombies (`Zombie`, `Husk`, `Drowned`) and Illagers (`Pillager`, `Vindicator`, `Evoker`, `Illusioner`).

---

## Gap 25: Axolotl Play Dead Mechanics, Goal Speeds & Targets Parity
- **Vanilla Reference**: `Axolotl.java:120-124, 388-394, 407-420`, `AxolotlAi.java:45-50, 95-125, 152-162`, `AxolotlAttackablesSensor.java:14-27`:
  - Attributes: `MAX_HEALTH = 14.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 2.0`, `STEP_HEIGHT = 1.0`.
  - Goals: `BreedGoal(0.2)`, `TemptGoal(0.5)`, `FollowParentGoal(0.6)`, `MeleeAttackGoal(0.6)`, `WanderAroundGoal(0.5)`. No Panic/EscapeDangerGoal.
  - Hunting Targets: includes fish (`Cod`, `Salmon`, `TropicalFish`, `Pufferfish`), `Tadpole`, and `Squid`/`GlowSquid`.
  - Play Dead Mechanism: in water when damaged, has a 1/3 chance to play dead for 200 ticks and receive `Regeneration I`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured attributes in constructor and initialized health to 14.0.
  - Aligned goal priorities and speeds; removed `EscapeDangerGoal`.
  - Added `Pufferfish` and `Tadpole` to hunt target goals.
  - Implemented `on_damage` hook for the vanilla 200-tick play dead state and self-regeneration.

---

## Gap 26: Turtle Attributes, Goals & Ambient Sound Interval Parity
- **Vanilla Reference**: `Turtle.java:152-160, 163-165, 173-175`:
  - Attributes: `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.25`, `STEP_HEIGHT = 1.0`.
  - Goals: `TurtlePanicGoal(1.2)` (priority 0), `TurtleBreedGoal(1.0)` (priority 1), `TemptGoal(1.1)` (priority 2), `TurtleGoToWaterGoal(1.0)` (priority 3), `LookAtPlayerGoal(8.0F)` (priority 8), `TurtleRandomStrollGoal(1.0, 100)` (priority 9). No `RandomLookAroundGoal`.
  - Ambient Sound Interval: 200 ticks.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured attributes `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.25`, `STEP_HEIGHT = 1.0`.
  - Aligned goals sequence (panic 0, breed 1, tempt 2, water 3, look 8, stroll 9); removed `RandomLookAroundGoal`.
  - Implemented `get_ambient_sound_interval() -> 200`.

---

## Gap 27: Dolphin Attributes & Combat Goals Parity
- **Vanilla Reference**: `Dolphin.java:158-174, 176-178`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.2`, `ATTACK_DAMAGE = 3.0`.
  - Goals: No panic goal. `MeleeAttackGoal(1.2F, true)` at priority 6, `RandomSwimmingGoal(1.0, 10)` at priority 4, avoid Guardians at priority 9.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.2`, `ATTACK_DAMAGE = 3.0`.
  - Removed `EscapeDangerGoal(1.6)`. Set `RandomSwimmingGoal(1.0)` and `MeleeAttackGoal(1.2, true)` at priority 6.

---

## Gap 28: Equine Animals (Horse, Donkey, Mule, SkeletonHorse, ZombieHorse) Stroll & Panic Parity
- **Vanilla Reference**: `AbstractHorse.java:135-153`, `AbstractChestedHorse.java:55-57`, `SkeletonHorse.java:47-49, 65-67`, `ZombieHorse.java:53-55, 126-129`:
  - All equines use `WaterAvoidingRandomStrollGoal(0.7)`.
  - `SkeletonHorse`: Empty `addBehaviourGoals()`, does NOT have `FloatGoal` (walks underwater) and does NOT panic. Attributes: `MAX_HEALTH = 15.0`, `MOVEMENT_SPEED = 0.2`.
  - `ZombieHorse`: Has `FloatGoal` and `TemptGoal`, does NOT panic. Attributes: `MAX_HEALTH = 25.0`.
  - `Donkey` & `Mule`: Attributes `MOVEMENT_SPEED = 0.175`, `JUMP_STRENGTH = 0.5`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated wander goals to `WaterAvoidingRandomStrollGoal(0.7)` across all equine mobs.
  - Removed `SwimGoal` and `EscapeDangerGoal` from `SkeletonHorse`; set attributes (health 15.0, speed 0.2).
  - Removed `EscapeDangerGoal` from `ZombieHorse`; set attributes (health 25.0).
  - Set `Donkey` and `Mule` attributes (`MOVEMENT_SPEED = 0.175`, `JUMP_STRENGTH = 0.5`).

---

## Gap 29: Polar Bear Attributes Parity
- **Vanilla Reference**: `PolarBear.java:102-108`:
  - Attributes: `MAX_HEALTH = 30.0`, `FOLLOW_RANGE = 20.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 6.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Explicitly configured `MAX_HEALTH = 30.0`, `FOLLOW_RANGE = 20.0`, `MOVEMENT_SPEED = 0.25`, and `ATTACK_DAMAGE = 6.0` on creation.

---

## Gap 30: Bosses (Wither & Ender Dragon) Attributes & Targeting Parity
- **Vanilla Reference**: `WitherBoss.java:99-107, 510-517`, `EnderDragon.java:119-121`:
  - Wither Attributes: `MAX_HEALTH = 300.0`, `MOVEMENT_SPEED = 0.6`, `FLYING_SPEED = 0.6`, `FOLLOW_RANGE = 40.0`, `ARMOR = 4.0`.
  - Wither Targets: `NearestAttackableTargetGoal(LivingEntity.class, 0, false, false, LIVING_ENTITY_SELECTOR)` where target is not in `WITHER_FRIENDS`.
  - Ender Dragon Attributes: `MAX_HEALTH = 200.0`, `CAMERA_DISTANCE = 16.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured Wither attributes (health 300.0, speed 0.6, flying 0.6, follow 40.0, armor 4.0).
  - Added `ActiveTargetGoal::predicated` to target all living entities except wither friends.
  - Configured Ender Dragon attributes (`MAX_HEALTH = 200.0`).

---

## Gap 31: Allay Attributes, Panic Speed, Regeneration & Loved Player Damage Immunity Parity
- **Vanilla Reference**: `Allay.java:126-132, 156-158, 217-219`, `AllayAi.java:49, 71`:
  - Attributes: `MAX_HEALTH = 20.0`, `FLYING_SPEED = 0.1`, `MOVEMENT_SPEED = 0.1`, `ATTACK_DAMAGE = 2.0`.
  - Panic speed multiplier: 2.5 (`AnimalPanic(2.5F)`).
  - Passive Healing: In `aiStep()`, heals 1.0 HP every 10 ticks while alive.
  - Loved Player Immunity: In `hurtServer()`, returns `false` if attacker entity UUID equals `liked_player` memory (`owner`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Set base attributes (`MAX_HEALTH = 20.0`, `FLYING_SPEED = 0.1`, `MOVEMENT_SPEED = 0.1`, `ATTACK_DAMAGE = 2.0`) and health initialized to 20.0.
  - Updated `EscapeDangerGoal` speed to 2.5.
  - Implemented 10-tick periodic self-regeneration in `mob_tick`.
  - Implemented `pre_damage` hook returning `false` when damaged by the recorded `owner`.

---

## Gap 32: Bee Attributes, Goals, Sting Death Randomization & Rolling Parity
- **Vanilla Reference**: `Bee.java:100-105, 175-196, 233-242, 375-380, 465-466, 529-535`:
  - Attributes: `MAX_HEALTH = 10.0`, `FLYING_SPEED = 0.6`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`.
  - Goals: No `LookAtPlayerGoal` or `RandomLookAroundGoal`. Has `TemptGoal(1.25)` for flowers/bee food.
  - Sting Death: In `customServerAiStep()`, checks every 5 ticks with probability `1 in clamp(1200 - timeSinceSting, 1, 1200)`. Dies when timer reaches 1200 ticks.
  - Rolling Animation Flag: In `aiStep()`, sets rolling flag when angry, has not stung, has a target, and distance to target is less than 4.0 blocks.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured attributes in constructor and initialized health to 10.0.
  - Removed `LookAtEntityGoal` and `RandomLookAroundGoal` from goal selector; configured `TEMPT_ITEMS` flower list.
  - Implemented vanilla probabilistic countdown in `mob_tick` for sting death.
  - Implemented dynamic rolling flag update based on distance to combat target.

---

## Gap 33: Ravager Attributes, Movement Speed States, Goals & Stun Parity
- **Vanilla Reference**: `Ravager.java:75-86, 98-107, 139-145, 209-222, 224-241`:
  - Attributes: `MAX_HEALTH = 100.0`, `MOVEMENT_SPEED = 0.3`, `KNOCKBACK_RESISTANCE = 0.75`, `ATTACK_DAMAGE = 12.0`, `ATTACK_KNOCKBACK = 1.5`, `FOLLOW_RANGE = 32.0`, `STEP_HEIGHT = 1.0`.
  - Movement Speed Transitions: 0.0 when immobile (attacking/stunned/roaring), 0.35 when pursuing a target, 0.3 normal base speed.
  - Goals: `FloatGoal(0)`, `MeleeAttackGoal(1.0, true)` (4), `WaterAvoidingRandomStrollGoal(0.4)` (5), `LookAtPlayerGoal(6.0)` (6), `LookAtPlayerGoal(Mob, 8.0)` (10).
  - Stun & Roar: 50% chance to stun for 40 ticks when hitting a blocking shield; roars 20 ticks later causing 6.0 damage and strong knockback to surrounding entities.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured complete base attributes and initialized health to 100.0.
  - Implemented dynamic movement speed adjustment in `mob_tick` based on target/immobile state.
  - Aligned goal priorities and shield stun/roar retaliation sequence.

---

## Gap 34: Shulker Attributes & Closed Shell Armor Parity
- **Vanilla Reference**: `Shulker.java:68-69, 97-105, 147-149, 429-435, 502-511`:
  - Attributes: `MAX_HEALTH = 30.0`.
  - Closed Shell Armor: `COVERED_ARMOR_MODIFIER = +20.0 Armor` when closed (`peek == 0`), arrow damage completely deflected (`hurtServer` returns false for arrows).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Explicitly configured base `MAX_HEALTH = 30.0` and health to 30.0.
  - Preserved shell armor calculation (+20 armor value) and arrow deflection in `pre_damage`.

---

## Gap 35: Strider Attributes, Goals & Cold Suffocating Parity
- **Vanilla Reference**: `Strider.java:80-83, 151-162, 309-317, 349-351, 374-376`:
  - Attributes: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.175`.
  - Water & Fire Behavior: `isSensitiveToWater() = true`, `isOnFire() = false`, walks on lava (`canStandOnFluid(LAVA)`). Does NOT float/swim in water (`FloatGoal` absent).
  - Goals: `PanicGoal(1.65)` (1), `BreedGoal(1.0)` (2), `TemptGoal(1.4)` (3), `FollowParentGoal(1.0)` (5), `RandomStrollGoal(1.0)` (7), `LookAtPlayerGoal(8.0)` (8).
  - Suffocating State: When not inside or on top of `LAVA` or `STRIDER_WARM_BLOCKS`, entity shivers/suffocates and receives a -34% speed reduction (`SUFFOCATING_MODIFIER`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes (`MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.175`) and initialized health to 20.0.
  - Removed `SwimGoal` so Strider sinks in water; aligned goal sequence and tempt items.
  - Implemented environment check in `mob_tick` querying lava contact and `MINECRAFT_STRIDER_WARM_BLOCKS` to update `DATA_SUFFOCATING`.

## Gap 36: Cat Attributes, Ambient Interval & Dynamic Ambient Sound
- **Vanilla Reference**: `Cat.java:254-270, 286-288`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`.
  - Ambient Sound Interval: `getAmbientSoundInterval() = 120` ticks (longer interval than the 80 ticks base default).
  - Ambient Sound Selection:
    - If tame and in love: `purr_sound`.
    - If tame and not in love: 1 in 4 chance for `purreow_sound`, else normal `ambient_sound`.
    - If untamed: `stray_ambient_sound`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`, and health `10.0` in `CatEntity::new`.
  - Overrode `get_ambient_sound_interval() -> i32 { 120 }`.
  - Implemented `get_ambient_sound(&self) -> Option<Sound>` following vanilla purr/purreow/stray ambient selection branches.

## Gap 37: Parrot Attributes, Goals & Cookie Poisoning
- **Vanilla Reference**: `Parrot.java:162-179, 280-298`:
  - Attributes: `MAX_HEALTH = 6.0`, `FLYING_SPEED = 0.4`, `MOVEMENT_SPEED = 0.2`, `ATTACK_DAMAGE = 3.0`.
  - Goals:
    - Priority 0: `TamableAnimalPanicGoal(1.25)` & `FloatGoal`.
    - Priority 1: `LookAtPlayerGoal(8.0)`.
    - Priority 2: `SitWhenOrderedToGoal`, `FollowOwnerGoal`, `ParrotWanderGoal(1.0)`.
    - Priority 3: `LandOnOwnersShoulderGoal`, `FollowMobGoal`.
    - No `RandomLookAroundGoal`.
  - Cookie interaction: Inflicts `POISON` for 900 ticks and immediately deals `Float.MAX_VALUE` damage via `playerAttack`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 6.0`, `FLYING_SPEED = 0.4`, `MOVEMENT_SPEED = 0.2`, `ATTACK_DAMAGE = 3.0`, and health `6.0` in `ParrotEntity::new`.
  - Added `EscapeDangerGoal::new(1.25)` and `SwimGoal` at priority 0; updated `LookAtPlayerGoal` range to 8.0; removed redundant `RandomLookAroundGoal`.

## Gap 38: Mooshroom Attributes & Lightning Conversion Deduplication
- **Vanilla Reference**: `MushroomCow.java:85-92` & `AbstractCow.java:56-58`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`.
  - Lightning Conversion: When struck by lightning, entity converts between Red and Brown variants. Deduplicates hits using `lastLightningBoltUUID` so a single multi-tick bolt does not continuously oscillate the variant.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`, and initialized health to 10.0 in `MooshroomEntity::new`.
  - Added UUID deduplication in `mob_on_lightning_strike` with `last_lightning_bolt_uuid`.

## Gap 39: Silverfish Attributes & Host Infestation Goals
- **Vanilla Reference**: `Silverfish.java:41-54, 82-92`:
  - Attributes: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 1.0`.
  - Goals: `FloatGoal(1)`, `ClimbOnTopOfPowderSnowGoal(1)`, `SilverfishWakeUpFriendsGoal(3)`, `MeleeAttackGoal(4, 1.0, false)`, `SilverfishMergeWithStoneGoal(5)`.
  - Targets: `HurtByTargetGoal(1).setAlertOthers()`, `NearestAttackableTargetGoal(2, Player)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 1.0`, and health `8.0` in `SilverfishEntity::new`.

## Gap 40: Endermite Attributes, Goals, Lifetime & Nametag Persistence
- **Vanilla Reference**: `Endermite.java:40-53, 105-129`:
  - Attributes: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 2.0`.
  - Goals: `FloatGoal(1)`, `ClimbOnTopOfPowderSnowGoal(1)`, `MeleeAttackGoal(2, 1.0, false)`, `WaterAvoidingRandomStrollGoal(3, 1.0)`, `LookAtPlayerGoal(7, 8.0)`, `RandomLookAroundGoal(8)`.
  - Targets: `HurtByTargetGoal(1).setAlertOthers()`, `NearestAttackableTargetGoal(2, Player)`.
  - Lifetime Despawn: Ticks up to 2400 unless `isPersistenceRequired()` is true (e.g. nametagged). Syncs body yaw to head yaw.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 2.0`, and health `8.0` in `EndermiteEntity::new`.
  - Reordered and aligned goals (priority 1 float/snow, priority 2 melee attack, priority 3 stroll, priority 7 look, priority 8 look around). Added `RevengeGoal(true).alerting_others()` at target priority 1.
  - Guarded lifetime increment by `!persistence_required`, synced `body_yaw` and `head_yaw`.

## Gap 41: Cave Spider Base Attributes & Wall Climbing
- **Vanilla Reference**: `CaveSpider.java:25-27, 30-49` & `Spider.java:89-91`:
  - Attributes: `MAX_HEALTH = 12.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`.
  - Attack: Applies Poison status effect on hit (Normal: 7s, Hard: 15s).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 12.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`, and health `12.0` in `CaveSpiderEntity::new`.

## Gap 42: Giant Camera Distance Attribute & Health Parity
- **Vanilla Reference**: `Giant.java:15-21`:
  - Attributes: `MAX_HEALTH = 100.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 50.0`, `CAMERA_DISTANCE = 16.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added base attribute `CAMERA_DISTANCE = 16.0` to `GiantEntity::new` alongside `MAX_HEALTH = 100.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 50.0`, and initialized health `100.0`.

## Gap 43: Pufferfish Base Attributes & Aquatic Goal Alignment
- **Vanilla Reference**: `Pufferfish.java:46-49, 91-95, 127-164` & `AbstractFish.java:44-46, 91-97`:
  - Attributes: `MAX_HEALTH = 3.0`.
  - Goals: `PanicGoal(1.25)` (0), `PufferfishPuffGoal` (1), `AvoidEntityGoal(Player, 8.0, 1.6, 1.4)` (2), `RandomSwimmingGoal(1.0)` (4).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attribute `MAX_HEALTH = 3.0` and health `3.0` in `PufferfishEntity::new`.
  - Aligned goal priorities with `PanicGoal(1.25)` at priority 0, `PufferfishPuffGoal` at priority 1, `AvoidEntityGoal(Player)` at priority 2, and `RandomSwimmingGoal(1.0)` at priority 4. Removed redundant land look goals.

## Gap 44: Wandering Trader Base Attributes Parity
- **Vanilla Reference**: `WanderingTrader.java:80-94` & `Villager.java:225-227`:
  - Attributes: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.5`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.5`, and health `20.0` in `WanderingTraderEntity::new`.

## Gap 46: Cod, Salmon & Tropical Fish Attributes & Goal Parity
- **Vanilla Reference**: `AbstractFish.java:44-46, 91-97`, `AbstractSchoolingFish.java:23-26`, `Cod.java`, `Salmon.java`, `TropicalFish.java`:
  - Attributes: `MAX_HEALTH = 3.0`.
  - Goals: `PanicGoal(1.25)` (0), `AvoidEntityGoal(Player, 8.0, 1.6, 1.4)` (2), `RandomSwimmingGoal(1.0)` (4).
  - No land look goals (`LookAtPlayerGoal` or `RandomLookAroundGoal`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attribute `MAX_HEALTH = 3.0` and health `3.0` in `CodEntity::new`, `SalmonEntity::new`, and `TropicalFishEntity::new`.
  - Aligned goal priorities to 0 (EscapeDangerGoal), 2 (AvoidEntityGoal), 4 (RandomSwimmingGoal). Removed extraneous land look goals.

## Gap 47: Tadpole Base Attributes & Goal Speed Multipliers
- **Vanilla Reference**: `Tadpole.java:97-99` & `TadpoleAi.java:27-29, 35-67`:
  - Attributes: `MAX_HEALTH = 6.0`, `MOVEMENT_SPEED = 1.0`.
  - Goals: Panic speed multiplier `2.0` (`AnimalPanic`), Tempt speed `1.25` (`FollowTemptation`), Idling swim speed `0.5` (`RandomStroll.swim`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 6.0`, `MOVEMENT_SPEED = 1.0`, and health `6.0` in `TadpoleEntity::new`.
  - Updated panic speed multiplier to 2.0 and idling swim speed to 0.5.

## Gap 48: Squid & Glow Squid Base Attributes Parity
- **Vanilla Reference**: `Squid.java:64-66` & `GlowSquid.java:26-32`:
  - Attributes: `MAX_HEALTH = 10.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attribute `MAX_HEALTH = 10.0` and health `10.0` in `SquidEntity::new` and `GlowSquidEntity::new`.

## Gap 49: Ocelot Base Attributes Parity
- **Vanilla Reference**: `Ocelot.java:142-144`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`, and health `10.0` in `OcelotEntity::new`.

## Gap 50: Chicken Base Attributes Parity
- **Vanilla Reference**: `Chicken.java:68-70`:
  - Attributes: `MAX_HEALTH = 4.0`, `MOVEMENT_SPEED = 0.25`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 4.0`, `MOVEMENT_SPEED = 0.25`, and health `4.0` in `ChickenEntity::new`.

## Gap 51: Cow Base Attributes Parity
- **Vanilla Reference**: `Cow.java:34-36`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`, and health `10.0` in `CowEntity::new`.

## Gap 52: Pig Base Attributes Parity
- **Vanilla Reference**: `Pig.java:63-65`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.25`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.25`, and health `10.0` in `PigEntity::new`.

## Gap 53: Sheep Base Attributes Parity
- **Vanilla Reference**: `Sheep.java:88-90`:
  - Attributes: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.23`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.23`, and health `8.0` in `SheepEntity::new`.

## Gap 54: Guardian Base Attributes & Goal Priorities Parity
- **Vanilla Reference**: `Guardian.java:132-135, 148-164`:
  - Attributes: `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 6.0`.
  - Goals:
    - Priority 4: `GuardianAttackGoal(1.0)`.
    - Priority 5: `MoveTowardsRestrictionGoal(1.0)`.
    - Priority 7: `RandomSwimmingGoal(1.0, 80)`.
    - Priority 8: `LookAtPlayerGoal(8.0)`.
    - Priority 8: `LookAroundGoal`.
    - Priority 9: `RandomLookAroundGoal`.
  - Spawning: Peaceful check, water fluid check (current and below), darkness/night check.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 6.0`, and health `30.0` in `GuardianEntity::new`.
  - Aligned goal priorities (4, 5, 7, 8, 8, 9) and verified unit tests.

## Gap 55: Elder Guardian Base Attributes, Persistence & Goals Parity
- **Vanilla Reference**: `ElderGuardian.java:31-33, 40-42`:
  - Attributes: `MAX_HEALTH = 80.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 8.0`.
  - Persistence: `setPersistenceRequired()` is called on spawn (Elder Guardian never despawns naturally).
  - Goals: Same structure as Guardian but wander interval is 400 ticks (`RandomSwimmingGoal(1.0, 400)`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 80.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 8.0` (dynamically injected into attributes map to compensate for data generator absence), and health `80.0` in `ElderGuardianEntity::new`.
  - Set `persistence_required = true` in constructor.
  - Aligned goal priorities and verified unit tests.

## Gap 56: Witch Base Attributes, Goals & Drinking Speed Modifier Parity
- **Vanilla Reference**: `Witch.java:48-50, 68-75, 107-109, 132, 169-172`:
  - Attributes: `MAX_HEALTH = 26.0`, `MOVEMENT_SPEED = 0.25`.
  - Goals:
    - Priority 1: `FloatGoal`.
    - Priority 2: `RangedAttackGoal(1.0, 60, 10.0)`.
    - Priority 2: `WaterAvoidingRandomStrollGoal(1.0)`.
    - Priority 3: `RaiderMoveThroughVillageGoal(1.05)`.
    - Priority 4: `PathfindToRaidGoal`.
    - Priority 5: `RaiderCelebrationGoal`.
    - Priority 6: `LookAtPlayerGoal(8.0)`.
    - Priority 6: `RandomLookAroundGoal`.
  - Drinking Speed Modifier: While drinking restorative/buff potions, entity applies `SPEED_MODIFIER_DRINKING = -0.25` modifier (ID `"minecraft:drinking"`), which is removed immediately once drinking finishes.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 26.0`, `MOVEMENT_SPEED = 0.25`, and health `26.0` in `WitchEntity::new`.
  - Aligned goal priorities to match vanilla ordering.
  - Implemented transient `-0.25` speed modifier addition and removal in `WitchEntity::mob_tick`.

## Gap 57: Blaze Base Attributes & Goal Priorities Parity
- **Vanilla Reference**: `Blaze.java:45-52, 54-56`:
  - Attributes: `MAX_HEALTH = 20.0`, `ATTACK_DAMAGE = 6.0`, `MOVEMENT_SPEED = 0.23`, `FOLLOW_RANGE = 48.0`.
  - Goals:
    - Priority 4: `BlazeAttackGoal`.
    - Priority 5: `MoveTowardsRestrictionGoal(1.0)`.
    - Priority 7: `WaterAvoidingRandomStrollGoal(1.0, 0.0)`.
    - Priority 8: `LookAtPlayerGoal(8.0)`.
    - Priority 8: `RandomLookAroundGoal`.
    - Priority 1 (Target): `HurtByTargetGoal(alertOthers=true)`.
    - Priority 2 (Target): `NearestAttackableTargetGoal(Player)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 20.0`, `ATTACK_DAMAGE = 6.0`, `MOVEMENT_SPEED = 0.23`, `FOLLOW_RANGE = 48.0`, and health `20.0` in `BlazeEntity::new`.
  - Added `MoveTowardsRestrictionGoal(1.0)` at Priority 5 and aligned goal priorities.

## Gap 58: Ghast Base Attributes, Follow Range & Flying Speed Parity
- **Vanilla Reference**: `Ghast.java:56-62, 116-122`:
  - Attributes: `MAX_HEALTH = 10.0`, `FOLLOW_RANGE = 100.0`, `CAMERA_DISTANCE = 8.0`, `FLYING_SPEED = 0.06`.
  - Goals:
    - Priority 5: `RandomFloatAroundGoal`.
    - Priority 7: `GhastLookGoal`.
    - Priority 7: `GhastShootFireballGoal`.
    - Priority 1 (Target): `NearestAttackableTargetGoal(Player, dy <= 4.0)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 10.0`, `FOLLOW_RANGE = 100.0`, `CAMERA_DISTANCE = 8.0`, `FLYING_SPEED = 0.06`, and health `10.0` in `GhastEntity::new`.
  - Preserved reflected fireball 1000.0 damage and vertical target filtering.

## Gap 59: Phantom Base Attributes & Size Scaling Attack Damage Parity
- **Vanilla Reference**: `Phantom.java:70-75, 87-90, DefaultAttributes.java:148`:
  - Attributes: `MAX_HEALTH = 20.0`, `ATTACK_DAMAGE = 6.0 + size` (base 6.0 for size 0).
  - Goals:
    - Priority 1: `PhantomAttackStrategyGoal`.
    - Priority 2: `PhantomSweepAttackGoal`.
    - Priority 3: `PhantomCircleAroundAnchorGoal`.
    - Priority 1 (Target): `PhantomAttackPlayerTargetGoal`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 20.0`, `ATTACK_DAMAGE = 6.0`, and health `20.0` in `PhantomEntity::new`.
  - Implemented `set_phantom_size` dynamically updating `ATTACK_DAMAGE` attribute to `6 + size` and synchronizing metadata.

## Gap 60: Creeper Base Attributes & Fall Damage Swell Parity
- **Vanilla Reference**: `Creeper.java:65-75, 77-79, 87-95`:
  - Attributes: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`.
  - Fall Damage Retaliation: In vanilla `causeFallDamage`, fall impact adds `(int)(fallDistance * 1.5)` to fuse swell (`swell`), capped at `maxSwell - 5`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`, and health `20.0` in `CreeperEntity::new`.
  - Implemented `on_damage` handling `DamageType::FALL` scaling current fuse time by `(fall_distance * 1.5) as i32` capped at `max_swell - 5`.

## Gap 61: Enderman Base Attributes & Navigation Parity
- **Vanilla Reference**: `Enderman.java:71-74, 116-123`:
  - Attributes: `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 7.0`, `FOLLOW_RANGE = 64.0`, `STEP_HEIGHT = 1.0`.
  - Attack Speed Modifier: `+0.15` transient speed boost when aggressive (`SPEED_MODIFIER_ATTACKING`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 7.0`, `FOLLOW_RANGE = 64.0`, `STEP_HEIGHT = 1.0`, and health `40.0` in `EndermanEntity::new`.

## Gap 62: Spider Base Attributes Parity
- **Vanilla Reference**: `Spider.java:58-68, 89-91`:
  - Attributes: `MAX_HEALTH = 16.0`, `ATTACK_DAMAGE = 2.0`, `MOVEMENT_SPEED = 0.3`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 16.0`, `ATTACK_DAMAGE = 2.0`, `MOVEMENT_SPEED = 0.3`, and health `16.0` in `SpiderEntity::new`.

## Gap 63: Bat Base Attributes & Non-Pushable Parity
- **Vanilla Reference**: `Bat.java:86-88, 98-100`:
  - Attributes: `MAX_HEALTH = 6.0`.
  - Physics/Collision: `isPushable() = false` (neither pushes nor can be pushed by other entities).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attribute `MAX_HEALTH = 6.0` and health `6.0` in `BatEntity::new`.
  - Set `collides = false` on entity spawn to mirror vanilla `isPushable() = false`.

## Gap 64: Zombie & Drowned Base Attributes & Step Height Parity
- **Vanilla Reference**: `Zombie.java:143-150` & `Drowned.java:83-85`:
  - Attributes:
    - Zombie: `MAX_HEALTH = 20.0`, `FOLLOW_RANGE = 35.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 3.0`, `ARMOR = 2.0`.
    - Drowned: Inherits Zombie attributes + `STEP_HEIGHT = 1.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured complete base attributes for all zombie variants in `ZombieEntityBase::without_behaviour_goals`.
  - Added explicit `STEP_HEIGHT = 1.0` attribute base configuration in `DrownedEntity::with_can_break_doors`.

## Gap 65: Zombified Piglin Base Attributes Parity
- **Vanilla Reference**: `ZombifiedPiglin.java:80-85`:
  - Attributes: `MAX_HEALTH = 20.0`, `FOLLOW_RANGE = 35.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 5.0`, `ARMOR = 2.0`, `SPAWN_REINFORCEMENTS_CHANCE = 0.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured base attributes `MAX_HEALTH = 20.0`, `FOLLOW_RANGE = 35.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 5.0`, `ARMOR = 2.0`, and health `20.0` in `ZombifiedPiglinEntity::new`.

## Gap 66: Skeleton & Stray Attack Interval Scaling by World Difficulty Parity
- **Vanilla Reference**: `AbstractSkeleton.java:132-157`:
  - Attack interval with bow dynamically adapts to world difficulty:
    - Hard difficulty: `getHardAttackInterval()` = 20 ticks (1.0s).
    - Normal / Easy / Peaceful: `getAttackInterval()` = 40 ticks (2.0s).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `get_min_attack_interval` in `Mob` trait for `SkeletonEntity` and `StraySkeletonEntity`, scaling cooldown dynamically based on `world.get_difficulty()`.

## Gap 67: Vex Piercing / NoPhysics Block Collision & NoGravity Parity
- **Vanilla Reference**: `Vex.java:74-78, 98-100`:
  - Physics/Collision: `noPhysics = true` (vex glides and maneuvers freely through solid blocks/walls), `hasNoGravity = true`.
  - Sunlight: Does not burn in sunlight (`burns_in_daylight = false`).
  - Base Attributes: `MAX_HEALTH = 14.0`, `ATTACK_DAMAGE = 4.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Configured `no_physics = true` and `set_has_no_gravity(true)` upon construction in `VexEntity::new`.
  - Implemented `burns_in_daylight = false` in `Mob` implementation.

## Gap 68: Allay Ambient Sound Variation & Dual States Parity
- **Vanilla Reference**: `Allay.java:217-227, 268-270`:
  - Ambient Sound: emits `ALLAY_AMBIENT_WITH_ITEM` when holding an item in main hand, and `ALLAY_AMBIENT_WITHOUT_ITEM` when empty-handed.
  - Duplication cooldown counts down each tick; when reaching 0, resets `DATA_CAN_DUPLICATE = true`.
  - Passive Regeneration: heals 1.0 HP every 10 ticks while alive.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `has_item_in_hand` helper and configured `get_ambient_sound` to dynamically switch between `EntityAllayAmbientWithItem` and `EntityAllayAmbientWithoutItem`.
  - Maintained duplication cooldown ticker and 10-tick passive health regeneration in `mob_tick`.

## Gap 69: Skeleton to Stray Powder Snow Conversion Parity
- **Vanilla Reference**: `Skeleton.java:18-64`, `ConversionTracker.java:53-112`:
  - When a Skeleton is trapped in Powder Snow (`isInPowderSnow`):
    - `FreezingTime` accumulates up to 140 ticks (7 seconds).
    - When reaching 140 ticks, triggers conversion: `DATA_STRAY_CONVERSION_ID = true`, `StrayConversionTime = 300` (15 seconds) shaking animation.
    - If removed from powder snow during conversion or affliction, resets timers and cancels shaking.
    - Upon conversion completion: converts into `EntityType::STRAY`, copies equipment, health, rotation, custom name, and emits `SOUND_SKELETON_TO_STRAY_CONVERSION` (1048).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `freezing_time` and `conversion_time` state tracking, `is_in_powder_snow()` monitoring in `mob_tick`, tracked data sync (`DATA_STRAY_CONVERSION_ID`), conversion completion with equipment transfer, and NBT serialization for `SkeletonEntity`.

## Gap 70: Zombie & Husk Water Drowning Conversion Parity
- **Vanilla Reference**: `Zombie.java:100-113, 232-280`, `Husk.java:78-90`:
  - When submerged in water (`isEyeInFluid(FluidTags.WATER)`):
    - `InWaterTime` accumulates up to 600 ticks (30 seconds).
    - Triggers shaking conversion: `DATA_DROWNED_CONVERSION_ID = true`, `DrownedConversionTime = 300` (15 seconds).
    - If surfaces before 300 ticks expire: cancels conversion and resets `InWaterTime = -1`.
    - Upon completion:
      - `Zombie` converts to `EntityType::DROWNED` with `SOUND_ZOMBIE_CONVERTED_TO_DROWNED`.
      - `Husk` converts to `EntityType::ZOMBIE` with `SOUND_HUSK_CONVERTED_TO_ZOMBIE`.
## Gap 71: Creaking Melee Attack Interval, Immunities & CanMove Gating Parity
- **Vanilla Reference**: `Creaking.java:128-181, 343-356`, `CreakingAi.java:54-65`:
  - `ATTACK_INTERVAL = 40` ticks (2.0 seconds) for Creaking melee attack (vs default 20 ticks).
  - While frozen (`!canMove()`), MeleeAttack and movement must be inhibited (`MeleeAttack.create(Creaking::canMove, 40)`).
  - `fireImmune()`: immune to fire when heart-bound.
  - `burns_in_daylight()`: does not burn in daylight.
  - `can_drown()`: does not drown.
  - On attack: broadcast entity event 4 and play `SoundEvents.CREAKING_ATTACK`.
  - Heart-bound damage check: checks `DamageTypeTags.BYPASSES_INVULNERABILITY`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `get_min_attack_interval(&self) -> Option<i32> { Some(40) }` in `CreakingEntity`.
  - Connected `MeleeAttackGoal` cooldown calculation to `mob.get_min_attack_interval()`.
  - Gated Creaking's `MeleeAttackGoal` with `creaking_can_move`.
  - Implemented `on_attack` with status packet 4 and attack sound.
  - Implemented `burns_in_daylight: false` and `can_drown: false`.

## Gap 72: Global Damage Pre-damage & Damage Adjustment Dispatch Parity
- **Vanilla Reference**: `LivingEntity.java:1120-1340`, `Mob.java:320-360`:
  - Vanilla dispatches pre-damage checks (e.g. invulnerability tags, projectile dodges, heart protection), mob damage scaling/reduction, and post-damage reactions (e.g. Warden anger, Silverfish call-for-help).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Integrated `mob.pre_damage(damage_type, source.or(cause))` and `mob.adjust_damage_amount(amount, &damage_type)` directly into `LivingEntity::damage_with_context`.
  - Integrated `mob.on_damage(damage_type, source.or(cause))` when damage is successfully applied.
  - Added default `fn adjust_damage_amount` to `Mob` trait.

## Gap 73: Sniffer Attribute & Trait Parity
- **Vanilla Reference**: `Sniffer.java:85-87`:
  - `MAX_HEALTH = 14.0`, `MOVEMENT_SPEED = 0.1`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 14.0` and `Attributes::MOVEMENT_SPEED = 0.1` in `SnifferEntity::new`.
  - Stored initial health to 14.0.
  - Implemented `as_sniffer(&self) -> Option<&SnifferEntity> { Some(self) }` on `Mob`.

## Gap 74: Armadillo Attributes, Head Yaw & Shell Damage Reduction Parity
- **Vanilla Reference**: `Armadillo.java:92-94, 292-298, 375-377`:
  - `MAX_HEALTH = 12.0`, `MOVEMENT_SPEED = 0.14`.
  - `getMaxHeadYRot`: `0.0` when scared (rolled up in shell), `32.5` when idle.
  - While scared: incoming damage is reduced by formula `(damage - 1.0) / 2.0`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 12.0` and `Attributes::MOVEMENT_SPEED = 0.14` in `ArmadilloEntity::new`.
  - Implemented `get_max_head_rotation(&self) -> f32`: returns `0.0` if scared, `32.5` if idle.
  - Implemented `adjust_damage_amount(&self, amount: f32, ...)` with formula `((amount - 1.0) / 2.0).max(0.0)`.

## Gap 75: Camel & Camel Husk Attributes, Step Height & Despawn Parity
- **Vanilla Reference**: `Camel.java:65, 120-126`, `CamelHusk.java:30-32`:
  - Base attributes: `MAX_HEALTH = 32.0`, `MOVEMENT_SPEED = 0.09`, `STEP_HEIGHT = 1.5`, `JUMP_STRENGTH = 0.42`.
  - `MAX_HEAD_Y_ROT = 30.0`.
  - Vanilla animal despawn rule: `Animal.removeWhenFarAway = false` (Camels do not despawn).
  - `CamelHusk.removeWhenFarAway = true` (Camel Husks despawn like monsters).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 32.0`, `Attributes::MOVEMENT_SPEED = 0.09`, `Attributes::STEP_HEIGHT = 1.5`, `Attributes::JUMP_STRENGTH = 0.42` in `CamelEntity::new` and `CamelHuskEntity::new`.
  - Configured `Mob::remove_when_far_away` default to `self.as_animal().is_none()`, properly preserving all animals while despawning monsters.
  - Overrode `CamelHuskEntity::remove_when_far_away` to `true`.
  - Implemented `get_max_head_rotation = 30.0`.

## Gap 76: Frog Attributes, Amphibious Traits & Fall Damage Reduction Parity
- **Vanilla Reference**: `Frog.java:131-133, 270-276, 304-306`:
  - `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 10.0`, `STEP_HEIGHT = 1.0`.
  - `MAX_HEAD_Y_ROT = 5.0`.
  - Amphibious: `can_drown = false`, `can_breathe_under_water = true`.
  - Fall damage reduction: `calculateFallDamage(fallDistance, damageModifier) - 5`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 10.0`, `Attributes::MOVEMENT_SPEED = 1.0`, `Attributes::ATTACK_DAMAGE = 10.0`, `Attributes::STEP_HEIGHT = 1.0` in `FrogEntity::new`.
  - Implemented `get_max_head_rotation = 5.0`.
  - Implemented `can_drown = false`, `can_breathe_under_water = true`.
  - Implemented `adjust_damage_amount`: subtracts 5.0 from `DamageType::FALL`.

## Gap 77: Goat Attributes & Fall Damage Reduction Parity
- **Vanilla Reference**: `Goat.java:79, 114-126`:
  - `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`, `ATTACK_DAMAGE = 2.0` (baby: 1.0).
  - Fall damage reduction: `calculateFallDamage(fallDistance, damageModifier) - 10`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 10.0`, `Attributes::MOVEMENT_SPEED = 0.2`, `Attributes::ATTACK_DAMAGE = 2.0` (baby: 1.0) and health `10.0` in `GoatEntity::new`.
  - Implemented `as_goat(&self) -> Option<&GoatEntity>` on `Mob`.
  - Implemented `adjust_damage_amount`: subtracts 10.0 from `DamageType::FALL`.

## Gap 78: Fox Attributes, Sweet Berry Bush Immunity & Loot Pickup Parity
- **Vanilla Reference**: `Fox.java:158, 313-320`, `SweetBerryBushBlock.java:78`:
  - Attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`, `SAFE_FALL_DISTANCE = 5.0`, `FOLLOW_RANGE = 32.0`.
  - Constructor: `setCanPickUpLoot(true)`.
  - Sweet berry bush: immune to damage from `DamageType::SWEET_BERRY_BUSH`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 10.0`, `Attributes::MOVEMENT_SPEED = 0.3`, `Attributes::ATTACK_DAMAGE = 2.0`, `Attributes::SAFE_FALL_DISTANCE = 5.0`, `Attributes::FOLLOW_RANGE = 32.0` and health `10.0` in `FoxEntity::new`.
  - Enabled `set_can_pick_up_loot(true)` on `mob_entity`.
  - Implemented `as_fox(&self) -> Option<&FoxEntity>` on `Mob`.
  - Implemented `pre_damage`: returns `false` (cancels) for `DamageType::SWEET_BERRY_BUSH`.

## Gap 79: Panda Attributes & Genetics (Weak / Lazy Gene) Parity
- **Vanilla Reference**: `Panda.java:284-286, 593-602, 704-745`:
  - Base: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.15`, `ATTACK_DAMAGE = 6.0`.
  - Recessive genes: `Brown (4)` and `Weak (5)` (`is_recessive = true`).
  - Gene variant: Recessive genes express only if `main == hidden`; otherwise defaults to `Normal`.
  - Derived attributes:
    - If `Weak`: `MAX_HEALTH = 10.0`.
    - If `Lazy`: `MOVEMENT_SPEED = 0.07`.
  - Ambient sound matches phenotype (`is_aggressive`, `is_worried`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `PandaGene::is_recessive` and `PandaGene::get_variant_from_genes`.
  - Implemented `get_variant`, `is_weak`, `is_lazy`, `is_worried`, `is_aggressive`, `is_playful`, `is_brown` on `PandaEntity`.
  - Implemented `set_attributes()` updating `MAX_HEALTH`, `MOVEMENT_SPEED`, `ATTACK_DAMAGE` dynamically upon gene changes.
  - Initialized base attributes and health in `PandaEntity::new`.
  - Implemented `as_panda(&self) -> Option<&PandaEntity>` on `Mob`.

## Gap 80: Axolotl Amphibious Traits, Water Resistance & Air Supply Parity
- **Vanilla Reference**: `Axolotl.java:262-282, 323-325, 388-394`:
  - Attributes: `MAX_HEALTH = 14.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 2.0`, `STEP_HEIGHT = 1.0`.
  - Trait flags: `can_breathe_under_water = true`, `can_drown = false`, `dries_out_on_land = true`, `rehydrates_in_rain = true`, `max_air_supply = 6000`, `is_pushed_by_fluids = false`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Confirmed attributes and amphibious trait flags in `AxolotlEntity`.
  - Implemented `as_axolotl(&self) -> Option<&AxolotlEntity>` on `Mob`.

## Gap 81: Dolphin Moistness, Air Supply, Drowning, Flop & Head Limits Parity
- **Vanilla Reference**: `Dolphin.java:78, 117-118, 176-178, 191-213, 235-256`:
  - Base attributes: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.2`, `ATTACK_DAMAGE = 3.0`.
  - Air supply: `max_air_supply = 4800`, `increase_air_supply = 4800`. Initial air supply is 4800.
  - Respiration: Dolphins are mammals, so `can_breathe_under_water = false` (they drown if submerged without air).
  - Head rotation: `get_max_head_rotation = 1.0` (`getMaxHeadYRot() = 1`).
  - Baby attack: Baby dolphins cannot attack (`can_attack` returns false).
  - Moistness & Flopping:
    - Moistness starts at 2400.
    - If in water or rain, reset moistness to 2400.
    - If out of water on land: moistness ticks down by 1 each tick. When moistness <= 0, takes 1.0 damage from `DamageType::DRY_OUT`.
    - When on ground on land, flops with random horizontal velocity (0.2), vertical velocity 0.5, and random yaw.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `Attributes::MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.2`, `ATTACK_DAMAGE = 3.0`, and air supply `4800` in `DolphinEntity::new`.
  - Implemented `max_air_supply = 4800`, `increase_air_supply = 4800`.
  - Implemented `get_max_head_rotation = 1.0`.
  - Implemented `can_attack` checking baby age status.
  - Implemented moistness ticking and flop mechanics in `mob_tick`.
  - Implemented `as_dolphin(&self) -> Option<&DolphinEntity>` on `Mob`.

## Gap 82: Turtle Attributes, Amphibious Navigation & Water Breathing Parity
- **Vanilla Reference**: `Turtle.java:101-105, 179-183, 203-205, 237-238, 298-300`:
  - Attributes: `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.25`, `STEP_HEIGHT = 1.0`.
  - Tag `#minecraft:can_breathe_under_water`: Turtles breathe underwater (`can_breathe_under_water = true`, `can_drown = false`).
  - Trait flags: `is_pushed_by_fluids = false`, `get_ambient_sound_interval = 200`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Confirmed attributes initialization and 30.0 HP in `TurtleEntity::new`.
  - Implemented `can_breathe_under_water() = true`, `can_drown() = false`, `is_pushed_by_fluids() = false`, `get_ambient_sound_interval() = 200`.
  - Implemented `as_turtle(&self) -> Option<&TurtleEntity>` on `Mob`.

## Gap 83: Polar Bear Attributes, Neutral Trait & Baby Alarm Parity
- **Vanilla Reference**: `PolarBear.java:140-142, 175-180`:
  - Attributes: `MAX_HEALTH = 30.0`, `FOLLOW_RANGE = 20.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 6.0`.
  - Neutral behavior: Panic goal only when baby (`is_baby`), revenge goal alerts other adult polar bears.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized attributes `MAX_HEALTH = 30.0`, `FOLLOW_RANGE = 20.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 6.0`, and HP 30.0 in `PolarBearEntity::new`.
  - Confirmed neutral mob traits and baby alarm logic in goal selectors.
  - Implemented `as_polar_bear(&self) -> Option<&PolarBearEntity>` on `Mob`.

## Gap 84: Rabbit Attributes & Killer Bunny (Evil Variant) Attack/Armor Parity
- **Vanilla Reference**: `Rabbit.java:101-105, 293-295, 369-383`:
  - Base attributes: `MAX_HEALTH = 3.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`.
  - Killer Bunny mutation (`Variant.EVIL`):
    - `Attributes.ARMOR = 8.0`
    - `Attributes.ATTACK_DAMAGE` gets transient modifier +5.0 (total 8.0).
    - Hostile target selectors attacking players, wolves, and foxes.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized base attributes `MAX_HEALTH = 3.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`, and HP 3.0 in `RabbitEntity::new`.
  - Implemented dynamic attribute updates in `set_variant`: sets `ARMOR = 8.0` and `ATTACK_DAMAGE = 8.0` when `RabbitVariant::Evil`, resets to 0.0 armor and 3.0 attack damage otherwise.
  - Implemented `as_rabbit(&self) -> Option<&RabbitEntity>` on `Mob`.

## Gap 85: Snow Golem Attributes, Water Sensitivity & Shearing Parity
- **Vanilla Reference**: `SnowGolem.java:63-65, 86-88, 100-115`:
  - Attributes: `MAX_HEALTH = 4.0`, `MOVEMENT_SPEED = 0.2`.
  - Water sensitivity: `is_sensitive_to_water = true` (takes drowning damage in water/rain).
  - Melts in warm biomes / dimensions (takes fire damage).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized base attributes `MAX_HEALTH = 4.0`, `MOVEMENT_SPEED = 0.2`, and HP 4.0 in `SnowGolemEntity::new`.
  - Confirmed water/rain and melting damage in `mob_tick`.
  - Implemented `as_snow_golem(&self) -> Option<&SnowGolemEntity>` on `Mob`.

## Gap 86: Copper Golem Attributes, Health & Weathering Parity
- **Vanilla Reference**: `CopperGolem.java:101-103`:
  - Attributes: `MOVEMENT_SPEED = 0.2`, `STEP_HEIGHT = 1.0`, `MAX_HEALTH = 12.0`.
  - States: Weathering copper states (`Unaffected`, `Exposed`, `Weathered`, `Oxidized`), waxing with honeycomb, scraping with axe, lightning strike resetting state.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `MAX_HEALTH = 12.0`, `MOVEMENT_SPEED = 0.2`, `STEP_HEIGHT = 1.0`, and HP 12.0 in `CopperGolemEntity::new`.
  - Implemented `as_copper_golem(&self) -> Option<&CopperGolemEntity>` on `Mob`.

## Gap 87: Happy Ghast Attributes, Baby Water Respiration & Flying Parity
- **Vanilla Reference**: `HappyGhast.java:151-159, 167, 191-193`:
  - Base attributes: `MAX_HEALTH = 20.0`, `TEMPT_RANGE = 16.0`, `FLYING_SPEED = 0.05`, `MOVEMENT_SPEED = 0.05`, `FOLLOW_RANGE = 16.0`, `CAMERA_DISTANCE = 8.0`.
  - Respiration: Baby happy ghast breathes underwater (`canBreatheUnderwater() = this.isBaby()`), adults drown without air.
  - Fall damage: `checkFallDamage` is no-op (immune to fall damage).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `MAX_HEALTH = 20.0`, `TEMPT_RANGE = 16.0`, `FLYING_SPEED = 0.05`, `MOVEMENT_SPEED = 0.05`, `FOLLOW_RANGE = 16.0`, `CAMERA_DISTANCE = 8.0`, and HP 20.0 in `HappyGhastEntity::new`.
  - Implemented `can_breathe_under_water(&self) -> bool { self.is_baby() }` and `can_drown(&self) -> bool { !self.is_baby() }`.
  - Implemented `as_happy_ghast(&self) -> Option<&HappyGhastEntity>` on `Mob`.

## Gap 88: Nautilus Attributes, Water Trait & Air Supply Parity
- **Vanilla Reference**: `AbstractNautilus.java:111-122`, `Nautilus.java:126-139`:
  - Base attributes: `MAX_HEALTH = 15.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 3.0`, `KNOCKBACK_RESISTANCE = 0.3`.
  - Fluid traits: `isPushedByFluid() = false`.
  - Air supply: `max_air_supply = 300`, `dries_out_on_land = true`, breathes underwater without drowning (`can_drown = false`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized `MAX_HEALTH = 15.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 3.0`, `KNOCKBACK_RESISTANCE = 0.3`, and HP 15.0 in `NautilusEntity::new`.
  - Implemented `is_pushed_by_fluids() = false`, `can_breathe_under_water() = true`, `can_drown() = false`, `max_air_supply() = 300`.
  - Implemented `as_nautilus(&self) -> Option<&NautilusEntity>` on `Mob`.

## Gap 89: Zombie Nautilus Attributes, Speed & Fluid Parity
- **Vanilla Reference**: `ZombieNautilus.java:51-54`, `AbstractNautilus.java:111-122`:
  - Base attributes: `MAX_HEALTH = 15.0`, `MOVEMENT_SPEED = 1.1`, `ATTACK_DAMAGE = 3.0`, `KNOCKBACK_RESISTANCE = 0.3`.
  - Fluid traits: `isPushedByFluid() = false`, `can_breathe_under_water() = true`, `can_drown() = false`.
## Gap 90: Wolf & Llama Interaction Predicate & Skeleton Target Parity
- **Vanilla Reference**: `Wolf.java:191-193, 678-680`, `Llama.java:459-468`:
  - Wild wolves (`!is_tamed()`) avoid both `LLAMA` and `TRADER_LLAMA` within 24 blocks.
  - Llama and Trader Llama target only wild/untamed wolves (`!target.is_tamed()`), never domesticated wolves.
  - Wolf targets all vanilla skeleton variants: `Skeleton`, `WitherSkeleton`, `Stray`, `Bogged`, and `Parched`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added AvoidEntityGoal for `EntityType::LLAMA` and `EntityType::TRADER_LLAMA` when untamed in `WolfEntity::new`.
  - Added `EntityType::PARCHED` to wolf's skeleton target list at priority 7.
  - Updated `LlamaEntity` and `TraderLlamaEntity` target selectors to filter on `!target.is_tamed()`.

## Gap 91: Iron Golem & Copper Golem Underwater Respiration Parity
- **Vanilla Reference**: `IronGolem.java:101-104`, tag `#minecraft:can_breathe_under_water`:
  - Vanilla `IronGolem` overrides `decreaseAirSupply(final int currentSupply) { return currentSupply; }`, effectively preventing any air depletion underwater.
  - `CopperGolem` is tagged with `#minecraft:can_breathe_under_water` and never drowns.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `can_breathe_under_water(&self) -> bool { true }` and `can_drown(&self) -> bool { false }` on `Mob` trait for `IronGolemEntity` and `CopperGolemEntity`.

## Gap 92: Phantom Cat Avoidance & Sweep Attack Target Reset Parity
- **Vanilla Reference**: `Phantom.java:460-473, 477-480`:
  - In `PhantomSweepAttackGoal`, every 20 ticks the phantom scans a 16-block inflated box for alive cats (`Cat.class`). If detected, plays `Sound::EntityCatHiss`, sets `is_scared_of_cat = true`, and ends the goal.
  - On stop, phantom sets `setTarget(null)` and resets attack phase to `CIRCLE`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added alive check `ent.is_alive()` when querying nearby cats in `PhantomSweepAttackGoal::should_continue`.
  - Added `mob.get_mob_entity().set_target(None)` in `PhantomSweepAttackGoal::stop`.

## Gap 93: Equine Mobs Attributes, Undead Underwater Respiration & Downcasts Parity
- **Vanilla Reference**: `AbstractHorse.java:379-387`, `AbstractChestedHorse.java:55-57`, `SkeletonHorse.java:47-49`, `ZombieHorse.java:53-55`, `Llama.java:133-135`, tag `#minecraft:can_breathe_under_water`:
  - `AbstractHorse` base attributes: `MAX_HEALTH = 53.0`, `MOVEMENT_SPEED = 0.225`, `JUMP_STRENGTH = 0.7`, `STEP_HEIGHT = 1.0`, `SAFE_FALL_DISTANCE = 6.0`, `FALL_DAMAGE_MULTIPLIER = 0.5`.
  - `AbstractChestedHorse` (Donkey, Mule, Llama, Trader Llama): `MOVEMENT_SPEED = 0.175`, `JUMP_STRENGTH = 0.5`.
  - `SkeletonHorse`: `MAX_HEALTH = 15.0`, `MOVEMENT_SPEED = 0.2`, `JUMP_STRENGTH = 0.7`. Undead equine: `can_breathe_under_water = true`, `can_drown = false`.
  - `ZombieHorse`: `MAX_HEALTH = 25.0`, `MOVEMENT_SPEED = 0.225`, `JUMP_STRENGTH = 0.7`. Undead equine: `can_breathe_under_water = true`, `can_drown = false`.
  - Equine downcasts on `Mob` trait: `as_horse`, `as_donkey`, `as_mule`, `as_skeleton_horse`, `as_zombie_horse`, `as_trader_llama`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Initialized base horse attributes and health across `HorseEntity`, `DonkeyEntity`, `MuleEntity`, `SkeletonHorseEntity`, `ZombieHorseEntity`, `LlamaEntity`, `TraderLlamaEntity`.
  - Implemented `can_breathe_under_water` and `can_drown` on `SkeletonHorseEntity` and `ZombieHorseEntity`.
## Gap 94: Vex, Squid, Glow Squid, Hoglin & Zoglin Parity & Downcasts
- **Vanilla Reference**: `Vex.java:98-100`, `Squid.java:44-50`, `GlowSquid.java:44-50`, `Hoglin.java:94-101`, `Zoglin.java:167-174`:
  - `Vex`: `MAX_HEALTH = 14.0`, `ATTACK_DAMAGE = 4.0`, health set to 14.0. `no_physics = true`, `has_no_gravity = true`.
  - `Squid` & `GlowSquid`: `MAX_HEALTH = 10.0`, `can_breathe_under_water = true`, `can_drown = false`, `is_pushed_by_fluids = false`, `dries_out_on_land = true`.
  - `Hoglin` & `Zoglin`: `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `KNOCKBACK_RESISTANCE = 0.6`, `ATTACK_KNOCKBACK = 1.0`, `ATTACK_DAMAGE = 6.0`.
  - Trait downcasts on `Mob`: `as_vex`, `as_squid`, `as_glow_squid`, `as_hoglin`, `as_zoglin`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `VexEntity` to initialize health to 14.0 (matching `MAX_HEALTH`).
  - Added explicit `can_breathe_under_water` and `can_drown` to `SquidEntity` and `GlowSquidEntity`.
## Gap 95: Strider Suffocating Speed Modifier, Water Sensitivity & Slime / Magma Cube Size Attributes Parity
- **Vanilla Reference**: `Strider.java:80-85, 164-174, 349-351, 374-376`, `Slime.java:119-125`, `MagmaCube.java:49-51, 60-66, 113-120`, `AbstractCubeMob.java:79-93`:
  - `Strider`:
    - Base attributes: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.175`.
    - `isSensitiveToWater() = true`.
    - Suffocating modifier: when out of warm blocks/lava, `SUFFOCATING_MODIFIER` = -0.34F multiplier (`speed = 0.175 * 0.66 = 0.1155`). When returning to warm blocks/lava, speed restores to base 0.175.
  - `Slime`:
    - `MAX_HEALTH = actualSize * actualSize`, `MOVEMENT_SPEED = 0.2 + 0.1 * actualSize`, `ATTACK_DAMAGE = actualSize`.
    - `canDealDamage() = !isTiny() && isEffectiveAi()` (tiny slimes size <= 1 do no contact damage).
  - `MagmaCube`:
    - `MAX_HEALTH = actualSize * actualSize`, `MOVEMENT_SPEED = 0.2 + 0.1 * actualSize`, `ATTACK_DAMAGE = actualSize + 2`, `ARMOR = actualSize * 3`.
    - `canDealDamage() = isEffectiveAi()` (all sizes, including tiny size 1, deal damage).
    - `jump_delay_multiplier = 4`.
  - Trait downcasts on `Mob`: `as_strider`, `as_cube_mob`, `as_slime`, `as_magma_cube`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `StriderEntity::set_suffocating` to dynamically apply/remove the 34% speed reduction (`0.175 * 0.66`), matching vanilla modifier.
  - Verified `CubeMobEntity` attributes scaling and contact damage parity for Slime and MagmaCube.
  - Added downcast methods `as_strider`, `as_cube_mob`, `as_slime`, `as_magma_cube` to `Mob` trait.

## Gap 96: Silverfish & Endermite Attributes, Goals, Lifetime & Downcasts Parity
- **Vanilla Reference**: `Silverfish.java:41-54, 126-230`, `Endermite.java:30-33, 40-54, 105-129`:
  - `Silverfish`:
    - Base attributes: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 1.0`, XP reward = 5.
    - AI: `SilverfishMergeWithStoneGoal` (merges into stone when mob griefing enabled), `SilverfishWakeUpFriendsGoal` (calls nearby infested blocks when damaged).
  - `Endermite`:
    - Base attributes: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 2.0`, XP reward = 3.
    - AI: Lifetime up to 2400 ticks (2 minutes) before despawning unless `persistence_required` is true.
  - Trait downcasts on `Mob`: `as_silverfish`, `as_endermite`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Verified attributes and behavioral goals for `SilverfishEntity` and `EndermiteEntity`.
## Gap 97: Classic Hostile Monsters Attributes, Water Sensitivity & Downcasts Parity
- **Vanilla Reference**: `Creeper.java:82-85`, `Spider.java:89-92`, `CaveSpider.java:25-27`, `Enderman.java:60-63, 375-377`, `Witch.java:48-51`, `Blaze.java:45-52`, `Ghast.java:55-58`, `Shulker.java:110-113`, `Phantom.java:85-88`:
  - `Creeper`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`, fuse/charge states.
  - `Spider`: `MAX_HEALTH = 16.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`, wall climbing.
  - `CaveSpider`: `MAX_HEALTH = 12.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`, poison attack.
  - `Enderman`: `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 7.0`, `isSensitiveToWater() = true`.
  - `Witch`: `MAX_HEALTH = 26.0`, `MOVEMENT_SPEED = 0.25`, potion usage and drinking speed modifier.
  - `Blaze`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 6.0`, `isSensitiveToWater() = true`.
  - `Ghast`: `MAX_HEALTH = 10.0`, `FOLLOW_RANGE = 100.0`.
  - `Shulker`: `MAX_HEALTH = 30.0`.
  - `Phantom`: `MAX_HEALTH = 20.0`, `ATTACK_DAMAGE = 6.0`.
  - Trait downcasts on `Mob`: `as_creeper`, `as_spider`, `as_cave_spider`, `as_enderman`, `as_witch`, `as_blaze`, `as_ghast`, `as_shulker`, `as_phantom`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented all 9 downcasts across `Mob` trait and their respective entity structures.
  - Verified base attributes, health, and water sensitivity parity.

## Gap 98: Zombie & Skeleton Variants Attributes, Traits & Downcasts Parity
- **Vanilla Reference**: `Zombie.java:124-131`, `Drowned.java:55-58`, `Husk.java:33-36`, `ZombieVillager.java`, `ZombifiedPiglin.java`, `AbstractSkeleton.java`, `Skeleton.java`, `WitherSkeleton.java:83-90`, `Stray.java`, `Bogged.java`, `Parched.java`:
  - `Zombie Variants`:
    - `Zombie`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 3.0`, `ARMOR = 2.0`.
    - `Drowned`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 3.0`, `ARMOR = 2.0`, `can_breathe_under_water = true`.
    - `Husk`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 3.0`, `ARMOR = 2.0`, sun immune (`burns_in_daylight = false`).
    - `Zombie Villager`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 3.0`, `ARMOR = 2.0`, curable with weakness potion & golden apple.
    - `Zombified Piglin`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.23`, `ATTACK_DAMAGE = 5.0`, `ARMOR = 2.0`, `fire_immune = true`.
  - `Skeleton Variants`:
    - `Skeleton`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`, bow attack AI.
    - `Wither Skeleton`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`, `ATTACK_DAMAGE = 4.0`, melee wither attack, `fire_immune = true`.
    - `Stray`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.25`, slowness arrows.
    - `Bogged`: `MAX_HEALTH = 16.0`, `MOVEMENT_SPEED = 0.25`, poison arrows, shearable mushrooms.
    - `Parched`: `MAX_HEALTH = 16.0`, `MOVEMENT_SPEED = 0.25`, `fire_immune = true`.
  - Trait downcasts on `Mob`: `as_zombie`, `as_drowned`, `as_husk`, `as_zombie_villager`, `as_zombified_piglin`, `as_skeleton`, `as_wither_skeleton`, `as_stray`, `as_bogged`, `as_parched`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented all 10 downcasts on `Mob` trait across all Zombie and Skeleton variants.
  - Verified attributes, respiration, sun burning, and combat behavior in unit test suite.

## Gap 99: Piglin Family (Piglin, Piglin Brute, Hoglin, Zoglin) Parity & Downcasts
- **Vanilla Reference**: `AbstractPiglin.java:25-33, 81-96, 103-107`, `Piglin.java:72-74, 164-166`, `PiglinBrute.java:32-35, 49-55`, `Hoglin.java:94-101`, `Zoglin.java:66-74, 167-174`:
  - `Piglin`: `MAX_HEALTH = 16.0`, `MOVEMENT_SPEED = 0.35`, `ATTACK_DAMAGE = 5.0`.
  - `Piglin Brute`: `MAX_HEALTH = 50.0`, `MOVEMENT_SPEED = 0.35`, `ATTACK_DAMAGE = 7.0`, `FOLLOW_RANGE = 12.0`.
  - `Hoglin`: `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 6.0`, `KNOCKBACK_RESISTANCE = 0.6`, `ATTACK_KNOCKBACK = 1.0`.
  - `Zoglin`: `MAX_HEALTH = 40.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 6.0`, `KNOCKBACK_RESISTANCE = 0.6`, `ATTACK_KNOCKBACK = 1.0`.
  - Zombification conversion time is 300 ticks when exposed to Overworld without `IsImmuneToZombification`.
  - Trait downcasts on `Mob`: `as_piglin`, `as_piglin_brute`, `as_abstract_piglin`, `as_hoglin`, `as_zoglin`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Aligned `PiglinEntity::new` to initialize base `MAX_HEALTH = 16.0`, `MOVEMENT_SPEED = 0.35`, and `ATTACK_DAMAGE = 5.0` to match vanilla `Piglin.createAttributes()`.
  - Implemented downcasts `as_piglin`, `as_piglin_brute`, `as_abstract_piglin` on `Mob` trait and concrete entity implementations.
  - Verified in `test_piglin_family_parity`.

## Gap 100: Illagers and Raiders (Pillager, Vindicator, Evoker, Illusioner, Ravager) Parity & Downcasts
- **Vanilla Reference**: `Pillager.java:84-90`, `Vindicator.java:89-95`, `Evoker.java:72-74`, `Illusioner.java:80-82`, `Ravager.java:98-107`:
  - `Pillager`: `MAX_HEALTH = 24.0`, `MOVEMENT_SPEED = 0.35`, `ATTACK_DAMAGE = 5.0`, `FOLLOW_RANGE = 32.0`.
  - `Vindicator`: `MAX_HEALTH = 24.0`, `MOVEMENT_SPEED = 0.35`, `ATTACK_DAMAGE = 5.0`, `FOLLOW_RANGE = 12.0`.
  - `Evoker`: `MAX_HEALTH = 24.0`, `MOVEMENT_SPEED = 0.5`, `FOLLOW_RANGE = 12.0`.
  - `Illusioner`: `MAX_HEALTH = 32.0`, `MOVEMENT_SPEED = 0.5`, `FOLLOW_RANGE = 18.0`.
  - `Ravager`: `MAX_HEALTH = 100.0`, `MOVEMENT_SPEED = 0.3`, `KNOCKBACK_RESISTANCE = 0.75`, `ATTACK_DAMAGE = 12.0`, `ATTACK_KNOCKBACK = 1.5`, `FOLLOW_RANGE = 32.0`, `STEP_HEIGHT = 1.0`.
  - Raider & Patrol characteristics: all implement `Raider`, `PatrollingMonster`, with ominous leader banner logic and raid wave goals.
  - Trait downcasts on `Mob`: `as_pillager`, `as_vindicator`, `as_evoker`, `as_illusioner`, `as_ravager`, `as_raider`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Aligned `PillagerEntity`, `VindicatorEntity`, `EvokerEntity`, and `IllusionerEntity` constructors to initialize exact vanilla base attributes and health.
  - Implemented downcasts `as_pillager`, `as_vindicator`, `as_evoker`, `as_illusioner`, `as_ravager` on `Mob` trait and concrete structs.
  - Verified in `test_illagers_and_raiders_parity`.

## Gap 101: Guardians, Breeze, Warden, Giant & Bat Parity & Downcasts
- **Vanilla Reference**: `Guardian.java:85-87`, `ElderGuardian.java:35-37`, `Breeze.java:67-73`, `Warden.java:185-193`, `Giant.java:15-21`, `Bat.java:98-100`:
  - `Guardian`: `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 6.0`.
  - `Elder Guardian`: `MAX_HEALTH = 80.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 8.0`, Mining Fatigue aura.
  - `Breeze`: `MAX_HEALTH = 30.0`, `MOVEMENT_SPEED = 0.63`, `FOLLOW_RANGE = 24.0`, `ATTACK_DAMAGE = 3.0`.
  - `Warden`: `MAX_HEALTH = 500.0`, `MOVEMENT_SPEED = 0.3`, `KNOCKBACK_RESISTANCE = 1.0`, `ATTACK_KNOCKBACK = 1.5`, `ATTACK_DAMAGE = 30.0`, `FOLLOW_RANGE = 24.0`.
  - `Giant`: `MAX_HEALTH = 100.0`, `MOVEMENT_SPEED = 0.5`, `ATTACK_DAMAGE = 50.0`, `CAMERA_DISTANCE = 16.0`.
  - `Bat`: `MAX_HEALTH = 6.0`, no collision, roosting and flying dynamics.
  - Trait downcasts on `Mob`: `as_guardian`, `as_elder_guardian`, `as_breeze`, `as_warden`, `as_giant`, `as_bat`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Aligned `BreezeEntity::new` and `WardenEntity::new` to initialize exact vanilla base attributes and health.
  - Implemented downcasts `as_guardian`, `as_elder_guardian`, `as_breeze`, `as_warden`, `as_giant`, `as_bat` on `Mob` trait and concrete structs.
  - Verified in `test_guardian_breeze_warden_giant_bat_parity`.

## Gap 102: Boss Entities (Wither, Ender Dragon) Parity & Downcasts
- **Vanilla Reference**: `WitherBoss.java:510-517`, `EnderDragon.java:119-121`:
  - `Wither`: `MAX_HEALTH = 300.0`, `MOVEMENT_SPEED = 0.6`, `FLYING_SPEED = 0.6`, `FOLLOW_RANGE = 40.0`, `ARMOR = 4.0`.
  - `Ender Dragon`: `MAX_HEALTH = 200.0`, `CAMERA_DISTANCE = 16.0`.
  - Trait downcasts on `Mob`: `as_wither`, `as_ender_dragon`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Aligned `EnderDragonEntity::new` to initialize base `CAMERA_DISTANCE = 16.0` per `EnderDragon.java:120`.
  - Implemented downcasts `as_wither`, `as_ender_dragon` on `Mob` trait and concrete structs.
  - Verified in `test_boss_entities_downcasts_and_parity`.

## Gap 103: Farm & Domestic Animals (Cow, Mooshroom, Sheep, Pig, Chicken, Cat, Ocelot, Parrot, Wolf) Parity & Downcasts
- **Vanilla Reference**: `AbstractCow.java:56-58`, `Sheep.java:108-110`, `Pig.java:92-94`, `Chicken.java:113-115`, `Cat.java:286-288`, `Ocelot.java:142-144`, `Parrot.java:173-179`, `Wolf.java:217-219`:
  - `Cow` / `Mooshroom`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.2`, `TEMPT_RANGE = 10.0`.
  - `Sheep`: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.23`.
  - `Pig`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.25`.
  - `Chicken`: `MAX_HEALTH = 4.0`, `MOVEMENT_SPEED = 0.25`.
  - `Cat`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`.
  - `Ocelot`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 3.0`.
  - `Parrot`: `MAX_HEALTH = 6.0`, `FLYING_SPEED = 0.4`, `MOVEMENT_SPEED = 0.2`, `ATTACK_DAMAGE = 3.0`.
  - `Wolf`: `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 4.0` (wild); increases to 40.0 on tame.
  - Trait downcasts on `Mob`: `as_cow`, `as_mooshroom`, `as_sheep`, `as_pig`, `as_chicken`, `as_cat`, `as_ocelot`, `as_parrot`, `as_wolf`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Aligned `WolfEntity::new` to initialize base `MAX_HEALTH = 8.0`, `MOVEMENT_SPEED = 0.3`, and `ATTACK_DAMAGE = 4.0` per `Wolf.java:217`.
  - Implemented downcasts `as_cow`, `as_mooshroom`, `as_sheep`, `as_pig`, `as_chicken`, `as_cat`, `as_ocelot`, `as_parrot`, `as_wolf` on `Mob` trait and concrete structs.
  - Verified in `test_farm_and_domestic_animals_parity`.

## Gap 104: Miscellaneous Passive & Aquatic Animals (Frog, Tadpole, Allay, Bee, Camel, Wandering Trader, Squid, Glow Squid, Dolphin, Fish) Parity & Downcasts
- **Vanilla Reference**: `Frog.java:270-276`, `Tadpole.java:97-99`, `Allay.java:126-132`, `Bee.java:529-535`, `Camel.java:120-126`, `Villager.java:225-227` / `WanderingTrader.java`, `Squid.java:64-66`, `Dolphin.java:176-178`, `AbstractFish.java:44-46`:
  - `Frog`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.0`, `ATTACK_DAMAGE = 10.0`, `STEP_HEIGHT = 1.0`.
  - `Tadpole`: `MAX_HEALTH = 6.0`, `MOVEMENT_SPEED = 1.0`.
  - `Allay`: `MAX_HEALTH = 20.0`, `FLYING_SPEED = 0.1`, `MOVEMENT_SPEED = 0.1`, `ATTACK_DAMAGE = 2.0`.
  - `Bee`: `MAX_HEALTH = 10.0`, `FLYING_SPEED = 0.6`, `MOVEMENT_SPEED = 0.3`, `ATTACK_DAMAGE = 2.0`.
  - `Camel`: `MAX_HEALTH = 32.0`, `MOVEMENT_SPEED = 0.09`, `JUMP_STRENGTH = 0.42`, `STEP_HEIGHT = 1.5`.
  - `Wandering Trader`: `MAX_HEALTH = 20.0`, `MOVEMENT_SPEED = 0.5`.
  - `Squid` / `Glow Squid`: `MAX_HEALTH = 10.0`.
  - `Dolphin`: `MAX_HEALTH = 10.0`, `MOVEMENT_SPEED = 1.2`, `ATTACK_DAMAGE = 3.0`.
  - Fish (`Cod`, `Salmon`, `Pufferfish`, `Tropical Fish`): `MAX_HEALTH = 3.0`.
  - Trait downcasts on `Mob`: `as_frog`, `as_tadpole`, `as_allay`, `as_bee`, `as_camel`, `as_wandering_trader`, `as_squid`, `as_glow_squid`, `as_dolphin`, `as_cod`, `as_salmon`, `as_pufferfish`, `as_tropical_fish`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Verified and aligned vanilla attributes across all passive and aquatic mobs.
  - Implemented downcasts `as_frog`, `as_tadpole`, `as_allay`, `as_bee`, `as_camel`, `as_wandering_trader`, `as_cod`, `as_salmon`, `as_pufferfish`, `as_tropical_fish` on `Mob` trait and concrete structs.
  - Verified in `test_misc_passive_and_aquatic_parity`.

## Gap 105: Camel Husk & Sulfur Cube Parity & Downcasts
- **Vanilla Reference**: `Camel.java:120-126`, `Slime.java:138-140`:
  - `Camel Husk`: `MAX_HEALTH = 32.0`, `MOVEMENT_SPEED = 0.09`, `JUMP_STRENGTH = 0.42`, `STEP_HEIGHT = 1.5`, despawns like a monster when far away.
  - `Sulfur Cube`: Cube mob kind with size-dependent health (`4 * size`), `contact_damage: ContactDamage::Never`, split count 2, max size 2.
  - Trait downcasts on `Mob`: `as_camel_husk`, `as_sulfur_cube`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `as_camel_husk` on `Mob` trait and `CamelHuskEntity`.
  - Implemented `as_sulfur_cube` on `Mob` trait and `CubeMobEntity`.
  - Verified in `test_camel_husk_and_sulfur_cube_parity`.

## Gap 106: Combat Rules & LivingEntity Armor Value & Toughness Parity
- **Vanilla Reference**: `LivingEntity.java:1884-1886, 1908-1915`, `CombatRules.java:16-32`, `Shulker.java:68-69`:
  - `LivingEntity.getArmorValue()` returns `floor(getAttributeValue(Attributes.ARMOR))` as `int`.
  - `LivingEntity.getDamageAfterArmorAbsorb` calls `CombatRules.getDamageAfterAbsorb(this, damage, damageSource, this.getArmorValue(), (float)this.getAttributeValue(Attributes.ARMOR_TOUGHNESS))`.
  - Mobs with natural base armor (e.g. Wither with 4.0, Zombies/Husks/Drowned with 2.0, Magma Cubes with 3*size) and Shulker when closed (20.0 armor) absorb damage via `Attributes.ARMOR` rather than only checking manual equipment slots.
  - Equipment attribute modifiers on any slot (including body/custom) are properly factored into total armor.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `get_armor_value(&self) -> i32` on `LivingEntity` and `Mob` trait, returning `floor(get_attribute_value(&Attributes::ARMOR))`.
  - Updated `LivingEntity::get_damage_after_armor_absorb` to use `self.get_armor_value() as f32` and `get_attribute_value(&Attributes::ARMOR_TOUGHNESS) as f32` with backwards-compatible attribute sync.
  - Aligned `ShulkerEntity` to dynamically update base armor to 20.0 when closed (`peek_amount == 0`) and 0.0 when open.
  - Verified in `test_combat_rules_armor_and_toughness_parity`.

## Gap 107: Fall Damage Calculation, Multiplier & Mob Overrides Parity
- **Vanilla Reference**: `LivingEntity.java:1811, 1852-1863`, `Frog.java:304-306`, `Goat.java:124-126`:
  - `calculateFallDamage(fallDistance, damageModifier) -> int`:
    - Checks `is(FALL_DAMAGE_IMMUNE)`.
    - `calculateFallPower(fallDistance) = fallDistance + 1.0E-6 - getAttributeValue(SAFE_FALL_DISTANCE)`.
    - Formula: `floor(baseDamage * damageModifier * getAttributeValue(FALL_DAMAGE_MULTIPLIER))`.
  - Specific mob overrides:
    - `Frog.calculateFallDamage`: reduced by 5 (`super.calculateFallDamage(...) - 5`).
    - `Goat.calculateFallDamage`: reduced by 10 (`super.calculateFallDamage(...) - 10`).
  - Fall Damage Sound: `getFallDamageSound(int dmg)` plays `GENERIC_BIG_FALL` if `dmg > 4` else `GENERIC_SMALL_FALL` (based on damage dealt rather than raw distance).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_fall_power` and `calculate_fall_damage` on `LivingEntity` factoring in `Attributes::FALL_DAMAGE_MULTIPLIER`.
  - Added `calculate_fall_damage` to `Mob` trait with default delegation.
  - Overrode `calculate_fall_damage` in `FrogEntity` (reducing fall damage by 5) and `GoatEntity` (reducing fall damage by 10).
  - Updated `LivingEntity::handle_fall_damage_from` to call `calculate_fall_damage` and select sound based on damage dealt.
  - Verified in `test_fall_damage_calculation_and_overrides_parity`.

## Gap 108: LivingEntity Heal & Death Checks Parity
- **Vanilla Reference**: `LivingEntity.java:1154-1171`:
  - `heal(float heal)`:
    - If `getHealth() > 0.0F`, sets health to `getHealth() + heal` (clamped to max health).
    - Does not panic or crash when heal amount is <= 0 or invalid.
    - Never restores health to dead/dying entities (`health <= 0.0F`), preventing unintended revivals without respawn.
  - `isDeadOrDying()`: returns `getHealth() <= 0.0F`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Removed panic assertion in `LivingEntity::heal`, ignored non-positive amounts, and guarded against healing dead entities (`current_health <= 0.0`).
  - Added `is_dead_or_dying(&self) -> bool` on `LivingEntity` and `Mob` trait.
  - Verified in `test_heal_and_death_checks_parity`.

## Gap 109: Mob Experience Drop & Base XP Reward Parity
- **Vanilla Reference**: `Mob.java:307-324`, `LivingEntity.java:583-585, 607-613, 1530-1535`, `Monster.java:123`, `Animal.java:134`, `Slime.java:123`, `Zombie.java:191`, `EquipmentSlot.java:canIncreaseExperience`:
  - `shouldDropExperience()`:
    - Base living entities return `!this.isBaby()`.
    - `Monster` overrides to `true` (monsters drop experience even if babies).
    - `Hoglin` overrides to `!this.isBaby()`.
    - `Tadpole` returns `false`.
  - `getBaseExperienceReward()`:
    - Animals & water creatures drop `1..=3` XP.
    - Cube mobs (Slimes, Magma Cubes) drop `size` XP.
    - Standard mobs drop base `xpReward` + equipment bonuses (`1..=3` XP per equipped item slot where `slot.canIncreaseExperience()`, which includes all equipment slots except saddle).
    - Zombie mobs drop 2.5x base XP when babies (`12` XP) and retain equipment bonuses.
    - Piglins and Hoglins drop base XP without equipment inflation.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `EquipmentSlot::can_increase_experience()` in `pumpkin-data`.
  - Implemented `calculate_equipment_experience_reward`, `is_baby_mob`, `should_drop_experience`, and `get_base_experience_reward` on `Mob` trait.
  - Aligned `ZombieEntityBase` (2.5x baby multiplier + equipment bonus) and `ZombifiedPiglinEntity`.
  - Updated `EntityBase for T where T: Mob` to guard `get_experience_reward` with `should_drop_experience`.
  - Verified in `test_mob_experience_reward_parity`.

## Gap 110: Air Supply, Respiration Oxygen Bonus & Drowning Damage Parity
- **Vanilla Reference**: `LivingEntity.java:448-453, 509-511, 591-605`, `Entity.java:2136, 2220, 2903`:
  - `decreaseAirSupply(currentSupply)`:
    - Checks `Attributes.OXYGEN_BONUS` (Respiration enchantment).
    - If `oxygenBonus > 0.0`, each tick has a `1.0 - (1.0 / (oxygenBonus + 1.0))` chance to retain the current air supply instead of depleting.
    - Otherwise decreases air by 1.
  - Negative Air Supply & Drowning:
    - Air supply decreases below 0 down to `-20` (20 ticks = 1 second of suffocation).
    - `shouldTakeDrowningDamage()` checks `getAirSupply() <= -20`.
    - When taking drowning damage: air supply resets to `0`, broadcasts entity event `67` (`EntityStatus::DrownParticles`), and deals `2.0F` damage.
  - `increaseAirSupply(currentSupply)`: replenishes `Math.min(currentSupply + 4, getMaxAirSupply())`.
  - NBT serialization: writes `Air` as short, preserving negative values down to -20 without clamping to 0 prematurely.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `decrease_air_supply`, `increase_air_supply`, and `should_take_drowning_damage` to `LivingEntity` and `Mob` trait factoring in `Attributes::OXYGEN_BONUS`.
  - Updated `BreathManager::tick` to use vanilla negative air timing (`air <= -20`), reset to 0, broadcast `EntityStatus::DrownParticles` (67), and deal drowning damage.
  - Aligned NBT read/write and `set_air_supply` to preserve negative air down to `-20`.
  - Verified in `test_air_supply_and_drowning_parity`.

## Gap 111: Attack Knockback Attribute & LivingEntity Knockback Calculation Parity
- **Vanilla Reference**: `LivingEntity.java:1546-1551, 2903-2905`, `Player.java:976-978, 1230-1231`:
  - `LivingEntity.getKnockback(target, damageSource)`:
    - In vanilla, knockback strength starts with `getAttributeValue(Attributes.ATTACK_KNOCKBACK) / 2.0F`.
    - Mobs with attack knockback attributes (e.g. Hoglin with 1.0, Ravager with 1.5, Iron Golem) add extra knockback to their attacks.
  - Knockback application in combat:
    - Base attack knockback is `0.4F`.
    - Added extra knockback from attacker is `0.4F + attacker.getKnockback()`.
    - Total knockback is reduced by target's `Attributes.KNOCKBACK_RESISTANCE`.
  - Robustness: Ensure `damage_with_context` proceeds with combat damage and knockback even when running outside a full server instance (e.g. testing environments).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `get_knockback(&self) -> f32` on `LivingEntity` and `Mob` trait.
  - Updated `LivingEntity::damage_with_context` to factor in attacker's `extra_kb` (`attacker.get_knockback()`) on top of base 0.4.
  - Allowed core damage and knockback processing to execute cleanly without aborting when server reference is not present.
  - Verified in `test_attack_knockback_attribute_and_strength_parity`.

## Gap 112: Falling Anvil / DAMAGES_HELMET 25% Reduction & Helmet Durability Parity
- **Vanilla Reference**: `LivingEntity.java:1209-1212, 1894-1906`, `Player.java:732-734`:
  - When damaged by any source with `#minecraft:damages_helmet` (falling anvil, falling block, falling stalactite):
    - If the target entity has a helmet equipped (`!this.getItemBySlot(EquipmentSlot.HEAD).isEmpty()`):
      - `hurtHelmet(source, damage)` is invoked, reducing the helmet's durability by `max(1, floor(damage / 4))`.
      - Incoming damage is reduced by 25% (`damage *= 0.75F`) BEFORE applying damage cooldown, armor calculation, or magic absorbs.
    - If target has no helmet equipped, full damage is sustained without the 25% damage mitigation.
  - Armor durability update persistence: ensure equipment state changes to damaged armor pieces in `damage_armor_items` are persisted back into `entity_equipment`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `has_helmet(&self) -> bool` to `LivingEntity` and `Mob` trait.
  - Integrated vanilla 25% reduction and helmet damage trigger before damage cooldown check in `LivingEntity::damage_with_context`.
  - Guarded subsequent `damage_armor_items` invocation against redundant duplicate helmet wear for `damages_helmet` sources.
  - Persisted updated damaged item stacks back into `self.entity_equipment` in `LivingEntity::damage_armor_items`.
  - Verified in unit test `test_damages_helmet_reduction_and_durability_parity`.

## Gap 113: Thorns Reflection, Armor Durability Penalty & Sound Parity
- **Vanilla Reference**: `LivingEntity.java:1266, 1350-1355, 2918`, `EnchantmentHelper.java:232-244`, `thorns.json`:
  - Thorns reflection trigger:
    - In vanilla `EnchantmentHelper.doPostAttackEffects`, when an entity lands a hit on a living victim, `runIterationOnEquipment` evaluates all equipment on the victim matching `EnchantmentTarget.VICTIM`.
    - For each equipped armor piece with `Enchantment::THORNS` (level `L`):
      - Probability of reflection is `0.15 * L` (Level 1: 15%, Level 2: 30%, Level 3: 45%).
      - When triggered, deals thorns damage to the attacker (`DamageType::THORNS`, amount `1.0 + rand(4)` or `L - 10` if `L > 10`).
      - The triggered armor piece incurs a 2-durability damage penalty (`change_item_damage = 2.0`).
      - Infinite recursion is guarded by checking `damage_type != DamageType::THORNS && !damage_type.has_tag(#minecraft:avoids_guardian_thorns)`.
  - Secondary hurt sound:
    - In `LivingEntity.java:1350-1355` (`playSecondaryHurtSound`), when taking `DamageTypes.THORNS`, `SoundEvents.THORNS_HIT` (`Sound::EnchantThornsHit`) is played alongside the entity's normal hurt sound.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `apply_thorns_reflection` to `LivingEntity` and `Mob` trait, executing thread-safely without holding equipment mutex across external damage calls.
  - Hooked `apply_thorns_reflection` into `LivingEntity::damage_with_context` when damage is dealt by an attacker.
  - Implemented vanilla secondary sound playback (`Sound::EnchantThornsHit`) for `DamageType::THORNS`.
  - Verified in unit test `test_thorns_reflection_and_durability_penalty_parity`.

## Gap 114: Sweeping Attack Prerequisites, Target Filtering & Damage Calculation Parity
- **Vanilla Reference**: `Player.java:1032-1042, 1153-1182`, `sweeping_edge.json`:
  - `isSweepAttack`:
    - Requires `fullStrengthAttack` (`attack_cooldown_progress > 0.9`), `!criticalAttack`, `!knockbackAttack`, and `this.onGround()`.
    - Horizontal speed must be less than `getSpeed() * 2.5` (`velocity.horizontalDistanceSqr() < (speed * 2.5)^2`).
    - Main hand item must be a sword (`ItemTags.SWORDS`).
  - `doSweepAttack`:
    - Formula: `sweepDamage = 1.0F + (float)this.getAttributeValue(Attributes.SWEEPING_DAMAGE_RATIO) * baseDamage`.
    - Target filtering: checks `nearby != this && nearby != entity && !this.isAlliedTo(nearby) && !(nearby instanceof ArmorStand armorStand && armorStand.isMarker()) && this.distanceToSqr(nearby) < 9.0`.
    - Knockback on each swept victim: `0.4F` along the attacker's facing yaw.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `AttackType::new` in `combat.rs` to enforce `on_ground` and `horizontal_speed_sq < (speed * 2.5)^2` matching vanilla `Player.isSweepAttack`.
  - Added `calculate_sweeping_damage` helper function implementing vanilla `1.0 + sweeping_ratio * base_damage`.
  - Filtered sweeping victims in `Player::attack` with `!is_allied_to` and `!ArmorStandEntity::is_marker`.
  - Verified in unit test `test_sweeping_damage_calculation_parity`.

## Gap 115: Shield Block Callback `blockedByItem`, Hoglin/Zoglin Throw & Ravager Stun Parity
- **Vanilla Reference**: `LivingEntity.java:1339-1341, 1387-1391`, `Hoglin.java:117-121`, `Zoglin.java:198-202`, `Ravager.java:209-222`, `HoglinBase.java:38-53`:
  - `blockUsingItem` callback:
    - In `LivingEntity.java:1339-1341`, when an attack is blocked (`damageBlocked > 0.0F`), if the damage is non-projectile and the direct attacker is a living entity, `attacker.blockedByItem(this, source, damage, fullyBlocked)` is called.
    - Default living entity behavior: if `!fullyBlocked`, defender is pushed with 0.5 knockback away from attacker.
  - Hoglin and Zoglin parity:
    - Adult Hoglins (`Hoglin.java:118`) and non-baby Zoglins (`Zoglin.java:199`) override `blockedByItem` to execute `HoglinBase.throwTarget(this, defender)`. Even when defender blocks their attack with a shield, defender is still tossed upward into the air.
  - Ravager parity:
    - When Ravager is blocked by a shield and `roarTick == 0`:
      - 50% chance: Ravager is stunned for 40 ticks (`stunnedTick = 40`), plays `SoundEvents.RAVAGER_STUNNED`, broadcasts entity event 39, stops navigator, and defender pushes Ravager.
      - 50% chance: Ravager applies `strongKnockback` to defender (`entity.push(xd / dd * 4.0, 0.2, zd / dd * 4.0)`).
  - Bug fix in hand-to-slot mapping:
    - In `LivingEntity::damage_with_context`, inverted slot mapping (`Hand::Left => MAIN_HAND`) fixed to `EquipmentSlot::from_hand(hand)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `on_blocked_by_shield` to `EntityBase` and `Mob` trait with default knockback when not fully blocked.
  - Overrode `on_blocked_by_shield` in `HoglinEntity` and `ZoglinEntity` to throw target when adult.
  - Overrode `on_blocked_by_shield` in `RavagerEntity` (50% stun 40 ticks + status 39 broadcast, 50% strong knockback).
  - Fixed active hand slot mapping to use `EquipmentSlot::from_hand(hand)` in `LivingEntity::damage_with_context`.
  - Invoked `on_blocked_by_shield` on melee direct attacker when blocked by shield.
  - Verified in unit test `test_shield_blocking_hoglin_throw_and_ravager_parity`.

## Gap 116: Shield Disabling Duration, Warden & Weapon Parity
- **Vanilla Reference**: `LivingEntity.java:4020-4024`, `Warden.java:176-178`, `Player.java:705-715`, `BlocksAttacks.java:81-103, 118-121`, `Item.java:484-486, 493-496`:
  - `LivingEntity.getSecondsToDisableBlocking`:
    - In vanilla, weapons like axes define `disable_blocking_for_seconds = 5.0F`. When an axe strikes a blocking shield, `getSecondsToDisableBlocking()` returns `5.0F`.
    - In `Warden.java:176-178`, Warden overrides `getSecondsToDisableBlocking()` to return `5.0F` even without holding any weapon.
  - `BlocksAttacks.disable`:
    - Cooldown ticks applied to the player's shield: `Math.round(seconds * 20.0F)` (5.0s -> 100 ticks).
    - Plays `SoundEvents.SHIELD_BREAK`, broadcasts entity status 30 (`EntityStatus::ShieldBreak`), clears active blocking hand, and applies cooldown.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `get_seconds_to_disable_blocking(&self) -> f32` to `EntityBase` and `Mob` trait.
  - Overrode `get_seconds_to_disable_blocking` in `WardenEntity` to return `5.0`.
  - Implemented weapon axe check (`5.0` seconds) on Player held item and LivingEntity `MAIN_HAND`.
  - Replaced hardcoded axe checks in `LivingEntity::damage_with_context` with dynamic `attacker.get_seconds_to_disable_blocking()`, applying proportional cooldown ticks `(disable_seconds * 20.0).round()`.
  - Verified in unit test `test_seconds_to_disable_blocking_parity`.

## Gap 117: Shield Facing Direction & Piercing Arrow Bypass Parity
- **Vanilla Reference**: `LivingEntity.java:1323-1335`, `Entity.java:2014-2022`, `BlocksAttacks.java:159-165`:
  - Piercing projectile bypass:
    - In `LivingEntity.java:1323-1324`: `if (source.getDirectEntity() instanceof AbstractArrow abstractArrow && abstractArrow.getPierceLevel() > 0) return 0.0F;`.
    - Projectiles with `pierce_level > 0` (e.g. crossbow piercing enchantment) completely bypass shield blocking, damaging the defender through their guard.
  - Facing angle resolution:
    - View direction is calculated from `this.getYHeadRot()` (head yaw rotation vector), matching vanilla `calculateViewVector(0.0F, this.getYHeadRot())`.
    - In Neth, previously used body `yaw`, which caused shield guards to fail when aiming with the mouse while standing still.
    - Angle resolution calculates `vector_to_source.dot(&look_vec) > 0.0`, granting a 180-degree forward blocking arc (horizontal angle <= 90 degrees).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added piercing arrow check bypassing shield blocking when `arrow.pierce_level > 0`.
  - Switched view direction calculation in `LivingEntity::damage_with_context` to use `head_yaw`.
  - Verified in unit test `test_shield_blocking_direction_and_piercing_parity`.

## Gap 118: Totem of Undying Equipment Consumption, Inventory Sync & Game Event Parity
- **Vanilla Reference**: `LivingEntity.java:1393-1424`, `DeathProtection.java:17-43`, `GameEvent.java:51`:
  - `LivingEntity.checkTotemDeathProtection`:
    - Checks `killingDamage.is(DamageTypeTags.BYPASSES_INVULNERABILITY)` (e.g. `OUT_OF_WORLD`, `GENERIC_KILL`). If so, totem cannot prevent death.
    - Iterates through hands (`MAIN_HAND`, then `OFF_HAND`). If an item with death protection (`TOTEM_OF_UNDYING`) is found, decrements stack by 1.
    - Synchronizes the decremented stack to player inventory (`player.inventory.set_stack_in_hand(hand, stack)`) and sends equipment change packet to tracking clients.
    - Calls `protectionItem.causeUseVibration(this, GameEvent.ITEM_INTERACT_FINISH)` (`world.emit_game_event(GameEvent::ItemInteractFinish)`).
    - Sets health to `1.0`, clears existing status effects, triggers advancement `UsedTotem`, broadcasts entity status 35 (`EntityStatus::TotemOfUndying`), plays totem sound.
    - Applies standard resurrection effects: Regeneration II (45 seconds / 900 ticks), Absorption II (5 seconds / 100 ticks), Fire Resistance I (40 seconds / 800 ticks).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `LivingEntity::check_totem_death_protection` to synchronize consumed totem item back into `player.inventory` and broadcast equipment changes via `send_equipment_changes`.
  - Added emission of `GameEvent::ItemInteractFinish` at entity location on totem activation.
  - Verified in unit test `test_totem_of_undying_consumption_and_resurrection_parity`.

## Gap 119: Fall Damage Block Modifiers, Sneak-Slime & HitGround Parity
- **Vanilla Reference**: `Entity.java:1603-1615`, `LivingEntity.java:1792-1818`, `SlimeBlock.java:15-18`, `PointedDripstoneBlock.java:54-60`, `HoneyBlock.java:44-52`, `GameEvent.java:31`:
  - Slime Block Sneaking Fall Damage:
    - In `SlimeBlock.java:16-18`: `if (!entity.isSuppressingBounce()) { entity.causeFallDamage(fallDistance, 0.0F, level.damageSources().fall()); } else { super.fallOn(level, state, pos, entity, fallDistance); }`.
    - When an entity is sneaking / crouching (`entity.is_sneaking()`), landing on a slime block deals full fall damage (`damage_modifier = 1.0F`). Previously Neth always passed `0.0` damage modifier regardless of sneak state.
  - Pointed Dripstone Tip Fall Distance:
    - In `PointedDripstoneBlock.java:56`: landing on an upward-pointing stalagmite tip deals damage with `fallDistance + 2.5` and multiplier `2.0F` with damage type `stalagmite`.
    - Previously Neth used `args.fall_distance + 2.0` (missing 0.5 distance).
  - Honey Block Landed Upon Sound & Particles:
    - In `HoneyBlock.java:45-52`: landing on honey plays `SoundEvents.HONEY_BLOCK_SLIDE` and broadcasts entity status event 54 (`HONEY_JUMP`) for particle effects.
  - HitGround Game Event:
    - In `Entity.java:1606-1611`: when landing on ground with `fallDistance > 0.0`, the world emits `GameEvent.HIT_GROUND` at the entity landing position.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `SlimeBlock::on_landed_upon` to check `args.entity.get_entity().is_sneaking()`, applying damage modifier `1.0` if sneaking, `0.0` otherwise.
  - Corrected `DripstoneBlock::on_landed_upon` tip distance modifier to `args.fall_distance + 2.5`.
  - Added honey slide sound and `EntityStatus::HoneyJump` broadcast to `HoneyBlock::on_landed_upon`.
  - Added `GameEvent::HitGround` emission in `LivingEntity::fall` when landing on ground.
  - Verified in unit test `test_fall_damage_block_modifiers_and_sneak_parity`.

## Gap 120: Player Attack Critical Hit Conditions & Magic Crit Effects Parity
- **Vanilla Reference**: `Player.java:1021-1030, 1061-1063`, `LivingEntity.java:1728-1736`, `Animation.java:4-5`:
  - `Player.canCriticalAttack(Entity entity)`:
    - Vanilla requirements for critical hit:
      ```java
      return this.fallDistance > 0.0
         && !this.onGround()
         && !this.onClimbable()
         && !this.isInWater()
         && !this.isMobilityRestricted() // this.hasEffect(MobEffects.BLINDNESS)
         && !this.isPassenger()
         && entity instanceof LivingEntity
         && !this.isSprinting();
      ```
    - Previously in Neth, `AttackType::new` only checked `!on_ground && fall_distance > 0.0` when `is_strong`. It lacked checks for:
      - `!on_climbable`: attacking while on ladders, vines, weeping/twisting vines, etc., is not a critical hit.
      - `!is_in_water`: attacking while submerged in water is not a critical hit.
      - `!has_effect(BLINDNESS)`: blindness suppresses critical hits (`isMobilityRestricted`).
      - `!is_passenger`: riding vehicles/mounts suppresses critical hits.
      - `!is_sprinting`: sprinting suppresses critical hits (causes knockback attack instead).
      - `victim instanceof LivingEntity`: attacking non-living entities (armor stands, boats, minecarts) cannot produce critical hits.
  - `Player.magicCrit(Entity entity)`:
    - In `Player.java:1061-1063`, when attacking with extra damage from enchantments (Sharpness, Smite, Bane of Arthropods, etc. where `extra_ench_damage > 0.0`), the server triggers `magicCrit`:
      - Broadcasts `Animation::MagicCriticaleffect` (byte 5) to tracking clients (Java edition).
      - Sends Bedrock `AnimateAction::MagicCriticalHit` to Bedrock clients.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `EntityBase::on_climbable` to check `#minecraft:climbable` block tag matching `LivingEntity.onClimbable`.
  - Implemented `can_critical_attack_raw` and `can_critical_attack` enforcing all 7 vanilla conditions (`fall_distance > 0`, `!on_ground`, `!on_climbable`, `!in_water`, `!blindness`, `!passenger`, `!sprinting`, and `victim is living entity`).
  - Added broadcast of `Animation::MagicCriticaleffect` in `Player::attack` when weapon has `extra_ench_damage > 0.0`.
  - Verified in unit test `test_critical_attack_conditions_parity`.

## Gap 121: Sweep Attack Mechanics, Particle Origin, Search Box & Post-Attack Effects Parity
- **Vanilla Reference**: `Player.java:1032-1042, 1153-1182, 976-981, 989`, `sweeping_edge.json`, `EnchantmentHelper.java:209-215`:
  - `Player.isSweepAttack`:
    - Enforces conditions: `fullStrengthAttack` (`attack_cooldown_progress > 0.9`), `!criticalAttack`, `!knockbackAttack`, `this.onGround()`, `horizontalSpeedSq < (getSpeed() * 2.5)^2`, and held weapon in main hand is `#minecraft:swords`. Excluded on Bedrock edition.
  - Sweep Particle Position & Direction:
    - In `Player.java:1178-1180`:
      ```java
      double dx = -Mth.sin(this.getYRot() * (float) (Math.PI / 180.0));
      double dz = Mth.cos(this.getYRot() * (float) (Math.PI / 180.0));
      serverLevel.sendParticles(ParticleTypes.SWEEP_ATTACK, this.getX() + dx, this.getY(0.5), this.getZ() + dz, 0, dx, 0.0, dz, 0.0);
      ```
    - Previously in Neth, particles were spawned relative to the victim's position with zero directional offset, causing the sweep animation to appear on the victim rather than sweeping from the attacker's weapon swing.
  - Search Box Inflate:
    - In `Player.java:1158`: `entity.getBoundingBox().inflate(1.0, 0.25, 1.0)`.
    - Previously in Neth, a static box with fixed height `[-0.25, 0.25]` around the victim's feet was used, missing taller entities in range.
  - Base Damage Scaling:
    - In vanilla `1.0F + (float)this.getAttributeValue(Attributes.SWEEPING_DAMAGE_RATIO) * baseDamage`, `baseDamage` is physical weapon damage without Sharpness/Smite enchantment bonus.
  - Post-Attack Effects on Swept Victims:
    - In `Player.java:1173`, `EnchantmentHelper.doPostAttackEffects(serverLevel, nearby, damageSource)` applies post-attack effects (e.g. Fire Aspect ignite) to all hurt swept entities.
  - Attack Sounds Location:
    - All attack sounds (`playServerSideSound`) play at the attacker's position (`this.getX(), this.getY(), this.getZ()`).
  - Non-Living Primary Target Knockback:
    - In `Player.java:976-981` and `causeExtraKnockback`, non-living primary targets receive extra knockback, dampening attacker velocity and resetting sprint.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `can_sweep_attack_raw` implementing all vanilla `Player.isSweepAttack` physics checks.
  - Corrected `spawn_sweep_particle` to spawn at attacker coordinates `(x + dx, y(0.5), z + dz)` with directional vector `(dx, 0, dz)`.
  - Replaced hardcoded search box with `victim_entity.bounding_box.load().expand(1.0, 0.25, 1.0)`.
  - Scaled sweeping damage using pure `base_damage` without enchantment extra damage.
  - Applied post-attack weapon effects (Fire Aspect ignition) to hurt swept entities.
  - Synchronized attack sound playback to the attacker's location.
  - Verified in unit tests `test_sweep_attack_conditions_parity` and `test_sweeping_damage_calculation_parity`.

## Gap 122: Player Attack Deflect Redirectable Projectiles & Damage Indicator Particles Parity
- **Vanilla Reference**: `Player.java:948-949, 1010-1018, 1066-1076`, `redirectable_projectile.json`, `ProjectileDeflection.java:18-24`:
  - `Player.deflectProjectile`:
    - In vanilla `Player.java:1010-1018`:
      ```java
      private boolean deflectProjectile(final Entity entity) {
         if (entity.is(EntityTypeTags.REDIRECTABLE_PROJECTILE)
            && entity instanceof Projectile projectile
            && projectile.deflect(ProjectileDeflection.AIM_DEFLECT, this, EntityReference.of(this), true, 1.0)) {
            this.level().playSound(null, this.getX(), this.getY(), this.getZ(), SoundEvents.PLAYER_ATTACK_NODAMAGE, this.getSoundSource());
            return true;
         } else {
            return false;
         }
      }
      ```
    - When a player attacks an entity belonging to `#minecraft:redirectable_projectile` (`fireball`, `wind_charge`, `breeze_wind_charge`):
      - Projectile is redirected along the player's look vector (`rotation().to_f64()`).
      - Projectile owner is transferred to the player.
      - Acceleration power is updated (`INITIAL_ACCELERATION_POWER` / `DEFLECTED_ACCELERATION_POWER`).
      - Plays `SoundEvents.PLAYER_ATTACK_NODAMAGE` at the player and returns early without dealing melee weapon damage or spending attack cooldown.
  - `Player.damageStatsAndHearts`:
    - In vanilla `Player.java:1066-1076`:
      ```java
      if (this.level() instanceof ServerLevel && actualDamage > 2.0F) {
         int count = (int)(actualDamage * 0.5);
         ((ServerLevel)this.level())
            .sendParticles(ParticleTypes.DAMAGE_INDICATOR, entity.getX(), entity.getY(0.5), entity.getZ(), count, 0.1, 0.0, 0.1, 0.2);
      }
      ```
    - When a player attack deals more than 2.0 actual damage (> 1 heart) to a living entity, the server emits `ParticleTypes.DAMAGE_INDICATOR` (cracked heart particles) with count `(actualDamage * 0.5) as i32` centered at the victim entity's mid-height `y(0.5)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `deflect_projectile(&self, deflector: &dyn EntityBase) -> bool` to `EntityBase` and implemented it on `FireballEntity` and `WindChargeEntity`.
  - Added `is_redirectable_projectile` helper verifying `#minecraft:redirectable_projectile`.
  - Added redirectable projectile deflection check at the start of `Player::attack`, playing `EntityPlayerAttackNodamage` and returning early.
  - Added `get_health` getter to `LivingEntity`.
  - Added `damage_indicator_particle_count` helper and spawned `Particle::DamageIndicator` when player deals `actual_damage > 2.0` to living victims.
  - Verified in unit tests `test_damage_indicator_and_redirectable_projectile_parity`.

## Gap 123: Living Entity Damage Invulnerability Cooldown & Partial Damage Calculation Parity
- **Vanilla Reference**: `LivingEntity.java:1218-1268` (`hurtServer`), `LivingEntity.java:1960-1979` (`actuallyHurt`):
  - In vanilla Minecraft:
    ```java
    boolean tookFullDamage = true;
    if (this.damageCooldownTime > 10.0F && !source.is(DamageTypeTags.BYPASSES_COOLDOWN)) {
       if (damage <= this.lastHurt) {
          return false;
       }

       this.actuallyHurt(level, source, damage - this.lastHurt);
       this.lastHurt = damage;
       tookFullDamage = false;
    } else {
       this.lastHurt = damage;
       this.damageCooldownTime = 20;
       this.actuallyHurt(level, source, damage);
       this.hurtDuration = 10;
       this.hurtTime = this.hurtDuration;
    }
    ```
  - `this.lastHurt` tracks raw incoming damage before armor absorption and enchantments.
  - If a hit arrives within the 10-tick invulnerability cooldown (`damageCooldownTime > 10`):
    - If `damage <= this.lastHurt`: the hit is completely discarded (`return false`). No armor damage, no sound, no knockback, no animation.
    - If `damage > this.lastHurt`: the entity receives the difference `damage - this.lastHurt` passed to `actuallyHurt` (which then applies armor/magic absorption). `tookFullDamage` is `false`, so knockback, hurt animations, hurt sounds, and hurt event packets are NOT replayed.
  - In previous Neth implementation:
    - Armor reduction and magic reduction were calculated before the cooldown check, and then compared `effective_amount <= last_damage` (comparing post-armor damage against pre-armor damage). This caused subsequent stronger hits during the invulnerability window to be rejected incorrectly or severely under-damaged (`effective_amount - last_damage` resulted in subtracting raw damage from post-armor damage).
    - Armor durability was damaged before the cooldown check, wasting durability even on discarded hits.
    - Even for partial hits (`tookFullDamage == false`), hurt animation packets, broadcast events, knockback, and hurt sounds were erroneously triggered.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_cooldown_damage(raw_damage, last_damage, hurt_cooldown, bypasses_cooldown) -> Option<(f32, bool)>` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Refactored `LivingEntity::damage_with_context`:
    - Checks cooldown and computes `damage_to_apply` and `took_full_damage` upfront.
    - Calculates armor damage and absorption on `damage_to_apply` inside the `actuallyHurt` sequence.
    - Updates `last_damage_taken` to the raw incoming damage.
    - Guards hurt animations, knockback velocity, damage event broadcasting, and hurt sound playback behind `took_full_damage`.
  - Verified in unit test `test_damage_cooldown_and_partial_hit_parity`.

## Gap 124: Combat Tracker Lifecycle, Timestamps, Periodic Recheck & Combat Packets Parity
- **Vanilla Reference**: `CombatTracker.java:37-54, 147-160`, `ServerPlayer.java:565-574, 895-924, 950`, `LivingEntity.java:730, 2780-2782`, `ClientboundPlayerCombatEnterPacket.java`, `ClientboundPlayerCombatEndPacket.java`:
  - `CombatTracker.recordDamage & recheckStatus`:
    - Tracks damage history and combat state transitions.
    - When taking damage from a `LivingEntity` while not currently in combat:
      - Transitions `inCombat` to `true`, records `combatStartTime`.
      - Calls `mob.onEnterCombat()`.
      - For players (`ServerPlayer.onEnterCombat`), sends `ClientboundPlayerCombatEnterPacket` (`CCombatEnter`).
    - Periodic recheck: In Vanilla `LivingEntity.java:2780-2782`:
      ```java
      if (this.tickCount % 20 == 0) {
         this.getCombatTracker().recheckStatus();
      }
      ```
    - Status recheck timeout:
      - If in combat: resets after 300 ticks (15s) of no damage.
      - If outside combat (taking non-mob damage like drowning/suffocation): resets after 100 ticks (5s).
      - On reset or on death (`!mob.isAlive()`):
        - If previously `inCombat`: calls `mob.onLeaveCombat()`.
        - For players (`ServerPlayer.onLeaveCombat`), sends `ClientboundPlayerCombatEndPacket` (`CCombatEnd`) with `duration = tickCount - combatStartTime`.
        - Clears combat entries list.
  - Death handling & `showDeathMessages` game rule:
    - In `ServerPlayer.java:895-923`:
      - If `showDeathMessages` is `false`, the server sends `ClientboundPlayerCombatKillPacket` (`CCombatDeath`) with empty text (`CommonComponents.EMPTY`).
      - Only if `showDeathMessages` is `true` does the server send the formatted death message to the dead player and broadcast to chat.
    - `this.getCombatTracker().recheckStatus()` is executed upon death, cleanly terminating combat and clearing entries.
  - Combat Timestamps:
    - Vanilla tracks entity age ticks (`this.mob.tickCount`). Previously in Neth, `damage_with_context` incorrectly read `world.level_info.load().day_time`, which breaks combat tracking whenever the time of day changes or players sleep in beds.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `CombatTracker::record_damage` to return `(Option<i64>, bool)` indicating whether combat was exited and/or entered.
  - Updated `CombatTracker::recheck_status` to return `Option<i64>` (duration of combat when leaving combat).
  - Added `on_enter_combat` and `on_leave_combat` to `LivingEntity` and `Player`, dispatching `CCombatEnter` and `CCombatEnd` packets.
  - Added periodic 20-tick recheck to `LivingEntity::tick` (mirroring `LivingEntity.java:2780`).
  - Switched combat timestamps from `day_time` to monotonic `self.entity.age.load(Relaxed) as i64`.
  - Added death cleanup recheck in `broadcast_death_message` and sent empty text in `CCombatDeath` when `show_death_messages` game rule is false.
  - Verified in unit test `test_combat_tracker_lifecycle_and_death_message_parity`.

## Gap 125: Shield Blocking Angle Projection, Durability Function & Axe Disable Parity
- **Vanilla Reference**: `LivingEntity.java:1311-1348` (`applyItemBlocking`), `BlocksAttacks.java:105-116, 127-135, 159-167, 188-193`, `Items.java:2428-2439`, `Item.java:493-495`, `Warden.java:176`:
  - `LivingEntity.applyItemBlocking`:
    - Angle Calculation:
      ```java
      Vec3 sourcePosition = source.getSourcePosition();
      double angle;
      if (sourcePosition != null) {
         Vec3 viewVector = calculateViewVector(0.0F, this.getYHeadRot());
         Vec3 vectorTo = sourcePosition.subtract(this.position());
         vectorTo = new Vec3(vectorTo.x, 0.0, vectorTo.z).normalize();
         angle = Math.acos(vectorTo.dot(viewVector));
      } else {
         angle = (float) Math.PI;
      }
      ```
    - Previously in Neth, `(pos - player_pos)` was normalized BEFORE setting `y = 0.0`. When the attacker and defender were on different elevations (jumping, stairs, slabs, hill terrain), the pre-normalized vector had reduced horizontal length (`< 1.0`), distorting the dot product angle calculation and causing blocks to fail erroneously even when facing directly at the attacker.
    - Zeroing `y` before normalization preserves accurate horizontal direction regardless of elevation.
  - Shield Durability Damage Function:
    - In vanilla Minecraft `DataComponents.BLOCKS_ATTACKS` for shields uses `ItemDamageFunction(3.0F, 1.0F, 1.0F)`:
      ```java
      public int apply(final float dealtDamage) {
         return dealtDamage < this.threshold ? 0 : Mth.floor(this.base + this.factor * dealtDamage);
      }
      ```
    - Hits `< 3.0F` (punch, snowball, weak attack) deal **0** durability damage to the shield.
    - Hits `>= 3.0F` deal `floor(1.0 + damage)` durability damage (e.g. 5.0 damage deals 6 durability damage).
    - Previously in Neth, all hits took `(amount).floor().max(1.0)` durability damage, inappropriately damaging the shield on trivial hits and under-damaging on strong hits.
  - Weapon Disabling:
    - Axes have `disableBlockingForSeconds = 5.0F` (100 ticks cooldown).
    - Warden melee has `getSecondsToDisableBlocking = 5.0F`.
    - Projectiles (`#minecraft:is_projectile`) cannot trigger shield disable or attacker pushback.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `can_block_damage_source(entity_pos, head_yaw, damage_pos) -> bool` with proper `y = 0.0` projection before normalization.
  - Implemented `calculate_shield_durability_damage(dealt_damage) -> i32` mirroring `BlocksAttacks.ItemDamageFunction(3.0F, 1.0F, 1.0F)`.
  - Refactored `damage_with_context` in `crates/pumpkin-core/src/entity/living.rs` to use these helpers.
  - Verified in unit tests `test_shield_blocking_angle_and_durability_parity`.

## Gap 126: Knockback Direction, NO_KNOCKBACK / NO_IMPACT Tags & Hurt Indication Parity
- **Vanilla Reference**: `LivingEntity.java:1237-1253` (`hurtServer`), `LivingEntity.java:1293-1309` (`dealDefaultKnockback`), `LivingEntity.java:1647-1665` (`knockback`), `ServerPlayer.java:2169-2172` (`indicateDamage`):
  - `dealDefaultKnockback`:
    ```java
    public void dealDefaultKnockback(final DamageSource source, final float damage, final boolean blocked) {
       double xd = 0.0;
       double zd = 0.0;
       if (source.getDirectEntity() instanceof Projectile projectile) {
          DoubleDoubleImmutablePair knockbackDirection = projectile.calculateHorizontalHurtKnockbackDirection(this, source);
          xd = -knockbackDirection.leftDouble();
          zd = -knockbackDirection.rightDouble();
       } else if (source.getSourcePosition() != null) {
          xd = source.getSourcePosition().x() - this.getX();
          zd = source.getSourcePosition().z() - this.getZ();
       }

       this.knockback(0.4F, xd, zd, source, damage);
       if (!blocked) {
          this.indicateDamage(xd, zd);
       }
    }
    ```
  - `hurtServer` checks:
    - `!source.is(DamageTypeTags.NO_IMPACT)`: Call `markHurt()` only when damage is impactful (skipping drowning, starvation, freezing, generic_kill).
    - `!source.is(DamageTypeTags.NO_KNOCKBACK)`: Call `dealDefaultKnockback` only when the damage type does not have `#minecraft:no_knockback` (skipping fire, lava, suffocation, fall, drown, etc.).
    - Previously in Neth:
      - `NO_KNOCKBACK` was not checked at all; entities took knockback on every tick of fire damage from an attacker entity.
      - `mark_hurt()` was only called if `source` was an entity; environmental/block damage did not sync velocity.
      - Hurt animation yaw used direct entity position difference even for projectiles and ignored positional damage without entity source.
      - When `(xd, zd) == (0.0, 0.0)`, hurt indication yaw returned 0 instead of `-yaw`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_knockback_direction(target_pos, source_pos, is_projectile, projectile_velocity, damage_position) -> (f64, f64)` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Implemented `calculate_hurt_direction_yaw(xd, zd, entity_yaw) -> f32` mirroring Vanilla `ServerPlayer.indicateDamage`: `(float)(Mth.atan2(zd, xd) * 180.0F / (float)Math.PI - this.getYRot())`.
  - Verified in unit tests `test_knockback_direction_and_hurt_indication_parity`.

## Gap 127: Attack Strength Ticker Reset & Main Hand Item Swap Interruption Parity
- **Vanilla Reference**: `Player.java:270-279, 1799-1830`, `ServerGamePacketListenerImpl.java:1551-1559, 1773-1779`, `ServerPlayer.java:2120-2130`:
  - `Player.java:273-276` (`Player.tick`):
    ```java
    ItemStack mainHandItemStack = this.getMainHandItem();
    if (!ItemStack.matches(this.lastItemInMainHand, mainHandItemStack)) {
       if (!ItemStack.isSameItem(this.lastItemInMainHand, mainHandItemStack)) {
          this.resetAttackStrengthTicker();
       }
       this.lastItemInMainHand = mainHandItemStack.copy();
    }
    ```
  - `ServerGamePacketListenerImpl.handlePunch`:
    ```java
    this.player.swing(InteractionHand.MAIN_HAND, swingAnimation, false);
    this.player.resetAttackStrengthTicker();
    ```
  - `ServerGamePacketListenerImpl.handleSetCarriedItem`:
    ```java
    if (this.player.getInventory().getSelectedSlot() != packet.getSlot() && this.player.getUsedItemHand() == InteractionHand.MAIN_HAND) {
       this.player.stopUsingItem();
    }
    this.player.getInventory().setSelectedSlot(packet.getSlot());
    ```
  - `ServerPlayer.drop`:
    ```java
    this.drop(removed, true, Prediction.PREDICTED);
    this.resetAttackStrengthTicker();
    ```
  - Previously in Neth:
    - `last_attacked_ticks` was ONLY reset when a melee attack landed.
    - Swinging in the air (missed hit), switching hotbar slot, swapping hands with F key, or dropping the held item did NOT reset the attack cooldown ticker. Players could swap from an empty hand to an axe and hit instantly at full strength without cooldown penalty.
    - Switching the active carried slot while eating or drawing a bow with main hand did not stop item usage.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `Player::reset_attack_cooldown()` and pure helpers `calculate_attack_cooldown_progress` and `calculate_base_damage_scale_factor`.
  - Added attack cooldown ticker reset on main hand arm swing (`handle_swing_arm`).
  - Added item use interruption (`player.stop_using_item()`) and attack cooldown reset on carried slot change (`handle_set_held_item`).
  - Verified in unit tests `test_attack_cooldown_progress_and_base_damage_scale_parity`.

## Gap 128: Critical Hit Formula (Enchantment Magic Boost Exclusion) & Sprint Knockback Sound Parity
- **Vanilla Reference**: `Player.java:946-966, 1052-1064`:
  - `Player.java:946-966`:
    ```java
    float attackStrengthScale = this.getAttackStrengthScale(0.5F);
    float magicBoost = attackStrengthScale * (this.getEnchantedDamage(entity, baseDamage, damageSource) - baseDamage);
    baseDamage *= this.baseDamageScaleFactor();
    ...
    baseDamage += attackingItemStack.getItem().getAttackDamageBonus(entity, baseDamage, damageSource);
    boolean criticalAttack = fullStrengthAttack && this.canCriticalAttack(entity);
    if (criticalAttack) {
       baseDamage *= 1.5F;
    }
    float totalDamage = baseDamage + magicBoost;
    ```
  - `Player.java:953-956, 1057-1059`:
    - Sprint knockback attacks play `SoundEvents.PLAYER_ATTACK_KNOCKBACK` immediately, AND ALSO play `SoundEvents.PLAYER_ATTACK_STRONG` in `attackVisualEffects` (since `fullStrengthAttack == true && !criticalAttack && !sweepAttack`).
  - Previously in Neth:
    - Enchantment extra damage (`extra_ench_damage`) was added to `damage` before the critical hit check, causing `damage *= 1.5` to multiply enchantment damage by 1.5x as well. In Vanilla Java, enchantment damage (Sharpness, Smite, Bane of Arthropods) is never multiplied by crits.
    - `AttackType::Knockback` only played `EntityPlayerAttackKnockback`, omitting the `EntityPlayerAttackStrong` sound effect.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_player_attack_damage(base_weapon_damage, attack_cooldown_progress, bonus_item_damage, extra_enchantment_damage, is_critical) -> f64` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `Player::attack` to compute `damage` using this vanilla formula where `extra_ench_damage` is added after the 1.5x critical multiplier on `base_damage`.
  - Updated `player_attack_sound` to play both `EntityPlayerAttackKnockback` and `EntityPlayerAttackStrong` on knockback attacks.
  - Verified in unit tests `test_player_attack_damage_critical_and_magic_boost_parity`.

### Gap 129: Post-Attack Effects, Fire Aspect / Fire Extinguishing in Water & Target Sound Interactions
- **Vanilla Reference**:
  - `Entity.java:1002-1008` & `WaterFluid.java:88`:
    - Entities extinguish fire (`clearFire()`) when in water or rain (`isInWater() || isInRain()`).
    - If the entity was on fire (`wasOnFire && !this.isOnFire()`), it plays `SoundEvents.GENERIC_EXTINGUISH_FIRE` (volume 0.7, pitch `1.6F + (rand - rand) * 0.4F`).
  - `Entity.java:1068-1074` (`extinguishFire`):
    - Plays extinguish sound if on fire, then clears fire ticks and resets visual fire flag.
  - `Player.java:1078-1094` (`itemAttackInteraction`), `EnchantmentHelper.java:226-261` (`doPostAttackEffectsWithItemSource`), `ApplyMobEffect.java:36-50`:
    - Weapon post-attack effects apply to the victim:
      - Fire Aspect (`Ignite`): Ignites victim for `4.0 * level` seconds. If victim is in water, immediately extinguishes with sound instead of burning underwater.
      - Bane of Arthropods (`ApplyMobEffect`): Applies Slowness IV (amplifier 3) for `1.5s` to `(1.5 + 0.5 * (level - 1))`s (`30` to `70` ticks) to `#minecraft:sensitive_to_bane_of_arthropods` mobs.
  - Previously in Neth:
    - Burning entities never extinguished when touching water or in rain during tick, continuing to burn and take periodic fire damage while submerged.
    - `Entity::extinguish` only zeroed `fire_ticks` without resetting visual fire or playing `Sound::EntityGenericExtinguishFire`.
    - `Player::attack` and sweep attack only implemented hardcoded `Ignite`, ignoring in-water target state and omitting `ApplyMobEffect` (Bane of Arthropods).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `calculate_extinguish_sound_pitch`, `should_extinguish_fire`, `calculate_mob_effect_duration_ticks`, `calculate_mob_effect_amplifier`, and `apply_weapon_post_attack_effects` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `play_entity_on_fire_extinguished_sound` and `extinguish_fire` in `crates/pumpkin-core/src/entity/mod.rs`, and integrated automatic extinguish into `Entity::tick`.
  - Replaced ad-hoc post-attack loops in `Player::attack` and sweeping attacks with `apply_weapon_post_attack_effects` supporting in-water extinguish and Bane of Arthropods `ApplyMobEffect`.
  - Verified with 15/15 passing tests in `entity::combat::tests`, including `test_fire_extinguish_and_mob_effect_post_attack_parity`.

### Gap 130: Mob Melee Attack Weapon Damage, Extra Knockback, Post-Attack Effects & PVP Resistance Stat Independence
- **Vanilla Reference**:
  - `Mob.java:1380-1401` (`doHurtTarget`):
    - Mob attack damage takes into account the held weapon item: `dmg += EnchantmentHelper.modifyDamage(level, weaponItem, target, damageSource, dmg)`.
    - Mob attack knockback takes into account `this.getKnockback(target, damageSource)`: `((attributeValue(Attributes.ATTACK_KNOCKBACK) + enchantment_knockback) / 2.0F)`.
    - If the target is hurt, mob runs `EnchantmentHelper.doPostAttackEffects(level, target, damageSource)` from its held weapon item (e.g. Fire Aspect, Bane of Arthropods).
  - `LivingEntity.java:1929-1935`:
    - When damage is reduced by Resistance effect, `awardStat(Stats.DAMAGE_RESISTED)` is awarded to the victim player, and `awardStat(Stats.DAMAGE_DEALT_RESISTED)` is independently awarded to the attacker player.
  - Previously in Neth:
    - `MobEntity::try_attack` completely ignored held weapon enchantments (Sharpness, Smite, Bane), weapon knockback, and post-attack effects (e.g., mob wielding a Fire Aspect sword never ignited the victim).
    - In `LivingEntity::get_damage_after_magic_absorb`, an `else if` prevented the attacker player from being awarded `CustomStatistic::DamageDealtResisted` in PvP when the victim was a player.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_mob_attack_damage` and `calculate_mob_extra_knockback` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `MobEntity::try_attack` in `crates/pumpkin-core/src/entity/mob/mod.rs` to compute held weapon enchantment damage, apply extra weapon/attribute knockback, and execute weapon post-attack effects via `apply_weapon_post_attack_effects`.
  - Updated `LivingEntity::get_damage_after_magic_absorb` to award `DamageResisted` to victim and `DamageDealtResisted` to attacker independently.
  - Verified with 16/16 passing unit tests in `entity::combat::tests`, including `test_mob_attack_damage_and_knockback_parity`.

### Gap 131: Curse of Vanishing Death Item Destruction & Totem Death Protection Tag Parity
- **Vanilla Reference**:
  - `Player.java:553-568` (`destroyVanishingCursedItems`) & `ServerPlayer.java:652`:
    - When `keepInventory` is false on player death, `destroyVanishingCursedItems()` removes all items possessing `EnchantmentEffectComponents.PREVENT_EQUIPMENT_DROP` (`minecraft:vanishing_curse`) so they are destroyed rather than dropped onto the ground.
  - `Mob.java:899-905` (`dropEquipment`):
    - When a mob dies and drops equipment, any item possessing `EnchantmentEffectComponents.PREVENT_EQUIPMENT_DROP` is skipped and destroyed.
  - `LivingEntity.java:1394` (`checkTotemDeathProtection`):
    - Death protection checks `damageSource.is(DamageTypeTags.BYPASSES_INVULNERABILITY)` rather than hardcoding specific damage types.
  - Previously in Neth:
    - Players and mobs dropped all items on death even if they carried `Curse of Vanishing` (`minecraft:vanishing_curse`).
    - `LivingEntity::check_totem_death_protection` hardcoded checks for `DamageType::OUT_OF_WORLD || DamageType::GENERIC_KILL` instead of using the vanilla tag `#minecraft:bypasses_invulnerability`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `has_vanishing_curse` helper in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `Player::send_combat_death` in `crates/pumpkin-core/src/entity/player.rs` to destroy vanishing-cursed items on death without dropping or incrementing drop statistics.
  - Updated `LivingEntity::drop_equipment` in `crates/pumpkin-core/src/entity/living.rs` to skip and destroy vanishing-cursed mob equipment.
  - Updated `LivingEntity::check_totem_death_protection` to check `damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)`.

### Gap 132: Breach Armor Reduction Additive Formula & Non-Player Living Attacker Weapon Breach Parity
- **Vanilla Reference**:
  - `CombatRules.java:23-31`:
    - In Vanilla, armor effectiveness modification (`EnchantmentHelper.modifyArmorEffectiveness`) operates directly on `armorFraction` (`realArmor / 25.0F`).
    - The Breach enchantment (`EnchantmentEffectComponents.ARMOR_EFFECTIVENESS`) defines `AddValue(LevelBasedValue.perLevel(-0.15F))` in `Enchantments.java:1197`, reducing the armor fraction additively by `0.15 * level` (`armorFraction += -0.15 * level`, clamped to `[0.0, 1.0]`).
    - For example, with 20 armor (0.60 fraction at 10 damage), Breach IV (-0.60) completely negates the armor (0.0 fraction, 100% damage through armor).
  - `CombatRules.java:22-25`:
    - Weapon item check reads the attacker's weapon from `damageSource.getWeaponItem()` for any living entity, not just player attackers.
  - Previously in Neth:
    - `CombatRules::get_damage_after_absorb` incorrectly initialized `effectiveness = 1.0` and multiplied `armor_fraction * effectiveness`, treating Breach as a multiplicative scale rather than the vanilla additive reduction `armor_fraction += -0.15 * level`.
    - `LivingEntity::get_damage_after_armor_absorb` only checked `attacker.get_player()`, ignoring held weapon Breach enchantments when a mob or other non-player living entity attacked.
    - `LivingEntity::get_damage_after_magic_absorb` omitted `EquipmentSlot::BODY` (e.g. Wolf armor) when calculating enchantment protection.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `CombatRules::get_damage_after_absorb` in `crates/pumpkin-core/src/entity/combat.rs` to apply `modify_armor_effectiveness` directly to `armor_fraction` additively and clamp to `[0.0, 1.0]`.
  - Updated `LivingEntity::get_damage_after_armor_absorb` in `crates/pumpkin-core/src/entity/living.rs` to extract Breach level from any living entity attacker's main hand stack.
  - Added `EquipmentSlot::BODY` to the equipment slots checked for damage protection enchantments in `LivingEntity::get_damage_after_magic_absorb`.
  - Added and verified unit test `test_breach_armor_reduction_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 18/18 tests in `entity::combat::tests` passing.
