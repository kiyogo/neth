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


