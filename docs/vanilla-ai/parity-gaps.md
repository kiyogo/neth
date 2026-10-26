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

### Gap 133: Weapon Attack Durability Consumption & ITEM_USED Stat Restricted to Living Victims Parity
- **Vanilla Reference**:
  - `Player.java:1086-1098` (`itemAttackInteraction`), `ItemStack.java:539-558` (`hurtEnemy` & `postHurtEnemy`):
    - When attacking an entity, `attackingItemStack.hurtEnemy(livingTarget, this)` and `postHurtEnemy(livingTarget, this)` are invoked **only** if the victim is a `LivingEntity` (`hurtTarget instanceof LivingEntity`).
    - If the item possesses the `DataComponents.WEAPON` component, `Stats.ITEM_USED.get(usedItem)` is awarded to the player attacker.
    - Non-living attack targets (such as boats, minecarts, and armor stands) never trigger weapon durability loss or award the `ITEM_USED` stat.
  - Previously in Neth:
    - In `Player::attack`, `self.damage_held_item(Self::combat_weapon_durability_cost(&item_stack))` was unconditionally called on every successful hit, damaging weapons even when attacking non-living entities (boats, minecarts, armor stands).
    - `Player::attack` never awarded the `StatisticCategory::Used` statistic when attacking with a weapon.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `should_weapon_damage_on_attack` helper in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `Player::attack` in `crates/pumpkin-core/src/entity/player.rs` to guard weapon durability reduction and `StatisticCategory::Used` increment behind `victim.get_living_entity().is_some()`.

### Gap 134: Jump Boost Safe Fall Distance & Slow Falling / Levitation Fall Damage Immunity Parity
- **Vanilla Reference**:
  - `LivingEntity.java:1862` (`calculateFallPower`) & `MobEffects.java:51` (`JUMP_BOOST`):
    - `MobEffects.JUMP_BOOST` registers an attribute modifier on `Attributes.SAFE_FALL_DISTANCE` with `1.0` per amplifier level (`Operation.ADD_VALUE`).
    - Every level of Jump Boost adds 1.0 block to the entity's safe fall distance (e.g. Jump Boost I provides 4.0 blocks safe fall, Jump Boost II provides 5.0 blocks).
  - `LivingEntity.java:3120-3122` (`travel`):
    - When an entity has `MobEffects.SLOW_FALLING` or `MobEffects.LEVITATION`, `resetFallDistance()` is called every tick.
    - Entities with Slow Falling or Levitation are completely immune to accumulating fall distance and taking fall damage.
  - Previously in Neth:
    - `calculate_fall_power` used only the base `SAFE_FALL_DISTANCE` attribute, omitting the `+1.0` per Jump Boost amplifier level.
    - `should_prevent_fall_damage` and `calculate_fall_damage` did not check `SLOW_FALLING` or `LEVITATION`, allowing entities to take fall damage on landing or when events evaluated fall damage.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `calculate_safe_fall_distance`, `calculate_fall_power`, and `calculate_fall_damage_raw` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `LivingEntity::should_prevent_fall_damage` in `crates/pumpkin-core/src/entity/living.rs` to return `true` when `SLOW_FALLING` or `LEVITATION` is active.
  - Updated `LivingEntity::calculate_fall_power` to incorporate the Jump Boost amplifier bonus (`+1.0` block per amplifier level).
  - Updated `LivingEntity::calculate_fall_damage` to return `0` when `SLOW_FALLING` or `LEVITATION` is active.

### Gap 135: Kill Credit Entity Age Tick Consistency & Tame Wolf Owner Player Attribution Parity
- **Vanilla Reference**:
  - `LivingEntity.java:1365-1376` (`resolvePlayerResponsibleForDamage`):
    - When an entity takes damage from a tame wolf (`wolf.isTame()`), the wolf's owner is credited as the player responsible (`setLastHurtByPlayer(wolf.getOwnerReference().getUUID(), 100)`).
  - `LivingEntity.java:1368` & `1445`:
    - The kill credit memory window is 100 ticks (5.0 seconds).
  - Previously in Neth:
    - In `LivingEntity::get_kill_credit`, the current time was read from `world.level_info.load().day_time`, but `last_hurt_by_player_time` stored `self.entity.age.load()`. Because `day_time` (e.g. 500,000) and `entity.age` (e.g. 100) are on entirely different time scales, `(current_tick - player_time).abs() <= 100` evaluated to false almost 100% of the time, breaking kill credit and player kill loot/XP drops.
    - If a tame wolf attacked an entity, the damage was credited only to the wolf as a mob (`last_hurt_by_mob_id`), never attributing player kill credit to the wolf's owner.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `is_kill_credit_active` helper in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `LivingEntity::get_kill_credit` in `crates/pumpkin-core/src/entity/living.rs` to consistently use `self.entity.age` tick count and `is_kill_credit_active`.
  - Updated `LivingEntity::damage_with_context` to credit the owner of a tame animal/wolf as `last_hurt_by_player_id` when the tame entity damages a target.
  - Added unit test `test_kill_credit_timing_and_tame_wolf_owner_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 21/21 tests in `entity::combat::tests` passing.

### Gap 136: Death Experience Drop Condition Parity & Player Death Experience Reward
- **Vanilla Reference**:
  - `LivingEntity.java:1529-1536` (`dropExperience`):
    ```java
    if (!this.wasExperienceConsumed()
       && (
          this.isAlwaysExperienceDropper()
             || this.lastHurtByPlayerMemoryTime > 0 && this.shouldDropExperience() && level.getGameRules().get(GameRules.MOB_DROPS)
       )) {
       ExperienceOrb.award(level, this.position(), this.getExperienceReward(level, killer));
    }
    ```
  - `Player.java:1610-1617` (`isAlwaysExperienceDropper` & `getBaseExperienceReward`):
    - Players are always experience droppers (`isAlwaysExperienceDropper() == true`), bypassing the player-kill requirement and `mobDrops` gamerule.
    - Player experience drop reward formula:
      `!keepInventory && !isSpectator ? Math.min(this.experienceLevel * 7, 100) : 0`
  - Previously in Neth:
    - In `LivingEntity::drop_all_death_loot`, awarding experience was unconditionally guarded by `params.killed_by_player.unwrap_or(false) && world.level_info.load().game_rules.mob_drops`.
    - Consequently, players who died from environmental damage (fall, lava, drowning, void, or mobs) never dropped experience orbs on death.
    - If `mobDrops` was disabled, dying players also never dropped experience orbs.
    - In `Player::get_experience_reward`, neither `keep_inventory` nor `is_spectator()` was checked, causing player death reward calculation to ignore gamerules and gamemode.
    - `has_player_kill` in `LivingEntity::drop_all_death_loot` did not check `get_kill_credit()`, preventing tame wolf kills or delayed player kills from triggering player-kill drops/experience.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `should_entity_drop_experience` and `calculate_player_death_experience_reward` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `LivingEntity::drop_all_death_loot` in `crates/pumpkin-core/src/entity/living.rs` to:
    - Incorporate `self.get_kill_credit()` in `has_player_kill`.
    - Mirror vanilla `LivingEntity.dropExperience`: distinguish player always-dropper status from mob `shouldDropExperience` and `mobDrops` gamerule.
  - Updated `Player::get_experience_reward` in `crates/pumpkin-core/src/entity/player.rs` to check `keep_inventory` and `is_spectator`.
  - Added unit test `test_experience_drop_conditions_and_player_death_reward_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 22/22 tests in `entity::combat::tests` passing.

### Gap 137: Mob Anger Damage Resolution, Tag Exemptions (`#minecraft:no_anger`, `#minecraft:no_anger_from_wind_charge`) & Player-as-Mob-Attacker Parity
- **Vanilla Reference**:
  - `LivingEntity.java:1235-1236, 1357-1363`:
    ```java
    protected void resolveMobResponsibleForDamage(final DamageSource source) {
       if (source.getEntity() instanceof LivingEntity livingSource
          && !source.is(DamageTypeTags.NO_ANGER)
          && (!source.is(DamageTypes.WIND_CHARGE) || !this.is(EntityTypeTags.NO_ANGER_FROM_WIND_CHARGE))) {
          this.setLastHurtByMob(livingSource);
       }
    }
    ```
  - In vanilla Minecraft Java, `resolvePlayerResponsibleForDamage` and `resolveMobResponsibleForDamage` are independent, non-exclusive steps:
    - Players are living entities (`Player instanceof LivingEntity`), so when a player attacks a living entity, both `setLastHurtByPlayer` and `setLastHurtByMob` are invoked.
    - Mob retaliation/anger and revenge AI targets rely on `last_hurt_by_mob_id` to aggro against the attacker.
    - Damage types with `#minecraft:no_anger` (e.g. `mob_attack_no_aggro`) must NOT trigger mob anger.
    - `wind_charge` damage against victims with `#minecraft:no_anger_from_wind_charge` (e.g. Breeze) must NOT trigger mob anger.
  - Previously in Neth:
    - In `LivingEntity::damage_with_context`, player attacker handling was placed in an `if ... else if` structure before `last_hurt_by_mob_id`. When a player attacked an entity, the `if attacker.get_player().is_some()` branch consumed the event, completely bypassing `last_hurt_by_mob_id.store(...)`.
    - Consequently, mobs attacked by players never registered the player in `last_hurt_by_mob_id`, breaking neutral mob revenge/anger AI targeting.
    - `#minecraft:no_anger` and `#minecraft:no_anger_from_wind_charge` tag checks were absent.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `can_cause_mob_anger` helper in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `LivingEntity::damage_with_context` in `crates/pumpkin-core/src/entity/living.rs` to decouple player attribution and mob anger resolution into two distinct phases, ensuring living player attackers are recorded in `last_hurt_by_mob_id` while respecting `#minecraft:no_anger` and `#minecraft:no_anger_from_wind_charge`.
  - Added unit test `test_mob_anger_resolution_and_tag_exemptions_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 23/23 tests in `entity::combat::tests` passing.

### Gap 138: Hurt Processing Parity - Directional Hurt Animation Suppression for NO_KNOCKBACK Damage
- **Vanilla Reference**:
  - `LivingEntity.java:1250-1253` & `dealDefaultKnockback` (`LivingEntity.java:1305-1308`):
    ```java
    boolean fullyBlocked = blocked && damage <= 0.0F;
    if (!source.is(DamageTypeTags.NO_KNOCKBACK) && !fullyBlocked) {
       this.dealDefaultKnockback(source, damage, blocked);
    }
    ```
    ```java
    public void dealDefaultKnockback(final DamageSource source, final float damage, final boolean blocked) {
       // ...
       this.knockback(0.4F, xd, zd, source, damage);
       if (!blocked) {
          this.indicateDamage(xd, zd);
       }
    }
    ```
  - `ServerPlayer.java:2169-2172` (`indicateDamage`):
    ```java
    @Override
    public void indicateDamage(final double xd, final double zd) {
       this.hurtDir = (float)(Mth.atan2(zd, xd) * 180.0F / (float)Math.PI - this.getYRot());
       this.connection.send(new ClientboundHurtAnimationPacket(this));
    }
    ```
  - In vanilla Minecraft Java, `ClientboundHurtAnimationPacket` (which tilts the player camera and renders directional damage yaw) is exclusively triggered via `dealDefaultKnockback`.
  - Damage types tagged `#minecraft:no_knockback` (such as `in_fire`, `on_fire`, `drown`, `starve`, `freeze`, `poison`, `wither`) bypass `dealDefaultKnockback`. They cause `broadcastDamageEvent` (red tint) and hurt sounds, but **never** send directional hurt animation or apply velocity knockback.
  - Previously in Neth:
    - `CHurtAnimation` and Bedrock `SActorEvent(ActorEventID::Hurt)` were sent unconditionally whenever `took_full_damage` was true, before and outside the `!damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK)` check.
    - As a result, environmental damage (fire ticks, drowning, hunger, poison, freeze) sent random-angled directional hurt animation packets every tick damage was applied, tilting the player's camera erratically.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `should_deal_default_knockback_and_hurt_anim` helper in `crates/pumpkin-core/src/entity/combat.rs`.
  - In `LivingEntity::damage_with_context` in `crates/pumpkin-core/src/entity/living.rs`, relocated the `CHurtAnimation` and hurt actor event broadcast inside the `!damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK)` block, mirroring vanilla `LivingEntity.dealDefaultKnockback`.
  - Added unit test `test_hurt_animation_and_knockback_no_knockback_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 24/24 tests in `entity::combat::tests` passing.

### Gap 139: Wither Rose Creation Upon Entity Death by Wither Boss Parity
- **Vanilla Reference**:
  - `LivingEntity.java:1480, 1494-1512` (`createWitherRose`) & `ServerPlayer.java:940`:
    ```java
    protected void createWitherRose(final @Nullable LivingEntity killer) {
       if (this.level() instanceof ServerLevel serverLevel) {
          boolean placed = false;
          if (killer instanceof WitherBoss) {
             if (serverLevel.getGameRules().get(GameRules.MOB_GRIEFING)) {
                BlockPos pos = this.blockPosition();
                BlockState state = Blocks.WITHER_ROSE.defaultBlockState();
                if (this.level().getBlockState(pos).isAir() && state.canSurvive(this.level(), pos)) {
                   this.level().setBlockAndUpdate(pos, state);
                   placed = true;
                }
             }

             if (!placed) {
                ItemEntity itemEntity = new ItemEntity(this.level(), this.getX(), this.getY(), this.getZ(), new ItemStack(Items.WITHER_ROSE));
                this.level().addFreshEntity(itemEntity);
             }
          }
       }
    }
    ```
  - `WitherRoseBlock.java:35-37`:
    ```java
    @Override
    protected boolean mayPlaceOn(final BlockState state, final BlockGetter level, final BlockPos pos) {
       return state.is(BlockTags.SUPPORTS_WITHER_ROSE);
    }
    ```
  - In vanilla Minecraft Java, whenever any living entity (including players) dies from damage credited to a Wither Boss:
    - If `mobGriefing` is enabled and the death block position is air with a valid supporting block below (`#minecraft:supports_wither_rose`), a Wither Rose block is planted in the world.
    - If placement fails or `mobGriefing` is disabled, a Wither Rose item stack is dropped at the death location.
  - Previously in Neth:
    - Neth lacked `createWitherRose` entirely; entities killed by a Wither boss never spawned or dropped Wither Roses.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `WitherRosePlacementAction` enum, `can_place_wither_rose`, and `determine_wither_rose_action` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Implemented `LivingEntity::create_wither_rose` in `crates/pumpkin-core/src/entity/living.rs` and integrated it into `drop_all_death_loot`.
  - Added unit test `test_wither_rose_creation_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 25/25 tests in `entity::combat::tests` passing.

### Gap 140: Living Entity & Player Death Lifecycle, Item Usage Cleanup, Sleep Cancellation & Death Velocity Parity
- **Vanilla Reference**:
  - `LivingEntity.java:1466-1470` (`die(DamageSource)`):
    ```java
    if (this.isSleeping()) {
       this.stopSleeping();
    }
    this.stopUsingItem();
    ```
  - `Player.java:534-548` & `ServerPlayer.java:947-949`:
    ```java
    if (source != null) {
       this.setDeltaMovement(
          -Mth.cos((this.getHurtDir() + this.getYRot()) * (float) (Math.PI / 180.0)) * 0.1F,
          0.1F,
          -Mth.sin((this.getHurtDir() + this.getYRot()) * (float) (Math.PI / 180.0)) * 0.1F
       );
    } else {
       this.setDeltaMovement(0.0, 0.1, 0.0);
    }
    this.clearFire();
    this.setTicksFrozen(0);
    this.setSharedFlagOnFire(false);
    ```
  - In vanilla Minecraft Java, whenever a living entity or player dies:
    - Any active item in use (shield blocking, bow/crossbow charging, eating, drinking) is immediately cancelled via `stopUsingItem()`.
    - Sleeping is immediately cancelled via `stopSleeping()`.
    - Player receives specific death knockback velocity (`(0.0, 0.1, 0.0)` if damage source is null, or rotated `(-cos(yaw)*0.1, 0.1, -sin(yaw)*0.1)` if damage source is present).
    - Player fire ticks and freeze ticks are cleared (`clearFire()`, `setTicksFrozen(0)`).
  - Previously in Neth:
    - `on_death` never called `stop_using_item()` or `clear_active_hand()`, leaving dead entities/players in active using item states.
    - Sleeping players were only awakened late in `handle_killed`.
    - Player death delta movement was not set to vanilla's standard death arc.
    - Fire ticks and freeze ticks were not extinguished upon player death.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_player_death_velocity` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added immediate item usage cleanup (`stop_using_item()` / `clear_active_hand()`), sleeping cancellation (`wake_up()`), and death delta velocity in `LivingEntity::on_death`.
  - Added `extinguish()` and `set_frozen_ticks(0)` upon player death in `Player::handle_killed`.
  - Added unit test `test_player_death_lifecycle_and_velocity_parity` in `combat.rs`. Total 26/26 tests in `entity::combat::tests` passing.

### Gap 141: Living Entity Stuck Arrows & Bee Stingers Tracking & Auto-Removal Parity
- **Vanilla Reference**:
  - `LivingEntity.java:2001-2015` (`getArrowCount`, `setArrowCount`, `getStingerCount`, `setStingerCount`):
    ```java
    public final int getArrowCount() { return this.entityData.get(DATA_ARROW_COUNT_ID); }
    public final void setArrowCount(final int count) { this.entityData.set(DATA_ARROW_COUNT_ID, count); }
    public final int getStingerCount() { return this.entityData.get(DATA_STINGER_COUNT_ID); }
    public final void setStingerCount(final int count) { this.entityData.set(DATA_STINGER_COUNT_ID, count); }
    ```
  - `AbstractArrow.java:468-470`:
    ```java
    if (mob != null) {
       mob.setArrowCount(mob.getArrowCount() + 1);
    }
    ```
  - `LivingEntity.java:2755-2778` (`tick()`):
    ```java
    int arrowCount = this.getArrowCount();
    if (arrowCount > 0) {
       if (this.removeArrowTime <= 0) {
          this.removeArrowTime = 20 * (30 - arrowCount);
       }
       this.removeArrowTime--;
       if (this.removeArrowTime <= 0) {
          this.setArrowCount(arrowCount - 1);
       }
    }
    int stingerCount = this.getStingerCount();
    if (stingerCount > 0) {
       if (this.removeStingerTime <= 0) {
          this.removeStingerTime = 20 * (30 - stingerCount);
       }
       this.removeStingerTime--;
       if (this.removeStingerTime <= 0) {
          this.setStingerCount(stingerCount - 1);
       }
    }
    ```
  - In vanilla Minecraft Java:
    - When an arrow strikes a living entity or player, its `arrowCount` tracked data is incremented, rendering visual arrows stuck in the entity's model.
    - During each tick, stuck arrows and stingers count down using an exact removal interval `20 * (30 - count)`. When the timer expires, the count decrements by 1 and a new timer begins if more projectiles remain.
    - On death, stuck projectiles are cleared (`arrowCount = 0`, `stingerCount = 0`).
  - Previously in Neth:
    - Neth lacked stuck arrows and bee stingers tracking, tracked data synchronization, hit increment, and decay timer mechanisms entirely.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Implemented `calculate_stuck_projectile_removal_time` and `tick_stuck_projectile_counter` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `arrow_count`, `stinger_count`, `remove_arrow_time`, and `remove_stinger_time` atomics, along with `get_arrow_count`, `set_arrow_count`, `get_stinger_count`, `set_stinger_count` in `crates/pumpkin-core/src/entity/living.rs` with `DATA_ARROW_COUNT_ID` and `DATA_STINGER_COUNT_ID` tracked data synchronization.
  - Added stuck projectile decay ticking in `LivingEntity::tick` and reset in `LivingEntity::on_death`.
  - Added arrow hit incrementation in `crates/pumpkin-core/src/entity/projectile/arrow.rs`.
  - Added unit test `test_stuck_projectiles_tracking_and_decay_parity` in `combat.rs`. Total 27/27 tests in `entity::combat::tests` passing.

### Gap 142: Hand Swap Animation Broadcast & Entity Status (Byte 55) Parity
- **Vanilla Reference**:
  - `LivingEntity.java:2989-3002` (`handleHandSwap`):
    ```java
    private void handleHandSwap(final Map<EquipmentSlot, ItemStack> changedItems) {
       ItemStack currentMainHand = changedItems.get(EquipmentSlot.MAINHAND);
       ItemStack currentOffHand = changedItems.get(EquipmentSlot.OFFHAND);
       if (currentMainHand != null
          && currentOffHand != null
          && ItemStack.matches(currentMainHand, this.lastEquipmentItems.get(EquipmentSlot.OFFHAND))
          && ItemStack.matches(currentOffHand, this.lastEquipmentItems.get(EquipmentSlot.MAINHAND))) {
          ((ServerLevel)this.level()).getChunkSource().sendToTrackingPlayers(this, new ClientboundEntityEventPacket(this, (byte)55));
          changedItems.remove(EquipmentSlot.MAINHAND);
          changedItems.remove(EquipmentSlot.OFFHAND);
          this.lastEquipmentItems.put(EquipmentSlot.MAINHAND, currentMainHand.copy());
          this.lastEquipmentItems.put(EquipmentSlot.OFFHAND, currentOffHand.copy());
       }
    }
    ```
  - In vanilla Minecraft Java:
    - Whenever items are swapped between main hand and off hand (such as pressing the 'F' key), the server broadcasts `ClientboundEntityEventPacket(entity, (byte)55)` (`EntityStatus::SwapHands`).
    - This triggers the visual hand swap item animation on all tracking clients.
  - Previously in Neth:
    - `Player::swap_item` only sent equipment slot changes and stopped using item, but never broadcasted `EntityStatus::SwapHands` (byte 55), leaving other players without the hand swap visual animation.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `is_equipment_hand_swap` helper and unit test `test_equipment_hand_swap_parity` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `EntityStatus::SwapHands` broadcast in `Player::swap_item` in `crates/pumpkin-core/src/entity/player.rs`. Total 28/28 tests in `entity::combat::tests` passing.

### Gap 143: Consumable Item Use Ticking, Eating/Drinking Sound & Particle Interval Parity
- **Vanilla Reference**:
  - `Consumable.java:119-124`:
    ```java
    public boolean shouldEmitParticlesAndSounds(final int useItemRemainingTicks) {
       int itemUsedForTicks = this.consumeTicks() - useItemRemainingTicks;
       int waitTicksBeforeUseEffects = (int)(this.consumeTicks() * 0.21875F);
       boolean isValidTime = itemUsedForTicks > waitTicksBeforeUseEffects;
       return isValidTime && useItemRemainingTicks % 4 == 0;
    }
    ```
  - `Consumable.java:104-117` (`emitParticlesAndSounds`):
    - Drink: volume `0.5`, pitch `Mth.randomBetween(random, 0.9F, 1.0F)`.
    - Eat: volume `random.nextBoolean() ? 0.5F : 1.0F`, pitch `random.triangle(1.0F, 0.2F)`.
    - Active ticking: spawns 5 item particles.
    - Final completion (`onConsume`): spawns 16 item particles and plays sound.
  - `PlaySoundConsumeEffect.java:27-30`:
    - Plays custom sound configured on consumable effect at volume `1.0` and pitch `1.0`.
  - In vanilla Minecraft Java:
    - Eating/drinking items does not emit sounds immediately upon right clicking; it waits until after `0.21875` fraction of the duration (7 ticks for standard 32-tick food), then emits chewing/drinking sounds and 5 item particles every 4 ticks.
    - Upon finishing consumption, it emits 16 particles and plays the final consume sound.
  - Previously in Neth:
    - Consumable item ticking only called `server.item_registry.on_use_tick` without emitting sound or particles at the vanilla intervals.
    - `ConsumeEffect::PlaySound` was a no-op stub (`=> {}`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `should_emit_consumable_use_effects` and `calculate_consumable_sound_volume_and_pitch` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `play_sound_event_fine` supporting custom volume and pitch in `crates/pumpkin-core/src/world/mod.rs`.
  - Added `emit_consumable_use_effects` and integrated eating/drinking ticking & completion sound and particles in `crates/pumpkin-core/src/entity/living.rs`.
  - Implemented `ConsumeEffect::PlaySound` in `LivingEntity::apply_consumable_effects`.
  - Added unit test `test_consumable_use_effects_and_sound_pitch_parity` in `combat.rs`. Total 29/29 tests in `entity::combat::tests` passing.

### Gap 144: Trident Riptide Launch Physics, Level Sounds, Auto Spin Attack State & Conditions Parity
- **Vanilla Reference**:
  - `TridentItem.java:69-70, 92-110, 129-130`:
    ```java
    float riptideStrength = EnchantmentHelper.getTridentSpinAttackStrength(itemStack, player);
    if (!(riptideStrength > 0.0F) || player.isInWaterOrRain() && !player.isPassenger()) {
       ...
       player.push(xd, yd, zd);
       player.startAutoSpinAttack(20, 8.0F, itemStack);
       if (player.onGround()) {
          player.move(MoverType.SELF, new Vec3(0.0, 1.1999999F, 0.0));
       }
       level.playSound(null, player, sound.value(), SoundSource.PLAYERS, 1.0F, 1.0F);
    }
    ```
  - `riptide.json`:
    - Spin attack strength formula: linear base `1.5`, per level above first `0.75` (Level 1: 1.5, Level 2: 2.25, Level 3: 3.0).
    - Sound effects: Level 1 `item.trident.riptide_1`, Level 2 `item.trident.riptide_2`, Level 3 `item.trident.riptide_3`.
  - `LivingEntity.java:3155-3158, 3261-3265`:
    - Auto spin attack runs for 20 ticks, setting flag bit 4 (`USING_RIPTIDE_FLAG`), enabling spin attack animation and pose.
  - In vanilla Minecraft Java:
    - Riptide can be used in water or rain (`isInWaterOrRain`) and cannot be used while riding vehicles (`!isPassenger`).
    - Using a Riptide trident on ground lifts the player by 1.2 blocks to clear ground drag.
    - Sound matches the exact Riptide level (1, 2, or 3).
  - Previously in Neth:
    - Checked only block `WATER` at player position, ignoring rain, bubble columns, or vehicle status.
    - Used inaccurate strength calculation `(1.0 + level * 0.75)`.
    - Never played Riptide sounds (only throw sound on plain throw).
    - Never set auto spin attack flag (bit 4) or timer for Java players, leaving client without spin animation.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `calculate_riptide_strength`, `calculate_riptide_velocity`, `get_riptide_sound`, and `can_perform_riptide` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `auto_spin_attack_ticks` atomic tracking, `start_auto_spin_attack`, decay countdown in `LivingEntity::tick`, and reset in `LivingEntity::on_death` in `crates/pumpkin-core/src/entity/living.rs`.
  - Updated `crates/pumpkin-core/src/item/items/trident.rs` with vanilla water/rain and vehicle checks, level sound dispatch, ground boost (`+1.2`), and auto spin attack activation.
  - Added unit test `test_riptide_strength_velocity_and_conditions_parity` in `combat.rs`. Total 30/30 tests in `entity::combat::tests` passing.

### Gap 145: Elytra Gliding Ticks, Durability Decay Interval & Fall Flying Auto-Exit Parity
- **Vanilla Reference**:
  - `LivingEntity.java:3181-3215`:
    ```java
    if (!this.level().isClientSide()) {
       if (!this.canGlide()) {
          this.setSharedFlag(7, false);
          return;
       }
       int checkFallFlyTicks = this.fallFlyTicks + 1;
       if (checkFallFlyTicks % 10 == 0) {
          int freeFallInterval = checkFallFlyTicks / 10;
          if (freeFallInterval % 2 == 0) {
             List<EquipmentSlot> slotsWithGliders = EquipmentSlot.VALUES.stream().filter(slot -> canGlideUsing(this.getItemBySlot(slot), slot)).toList();
             EquipmentSlot slotToDamage = Util.getRandom(slotsWithGliders, this.random);
             this.getItemBySlot(slotToDamage).hurtAndBreak(1, this, slotToDamage);
          }
          this.gameEvent(GameEvent.ELYTRA_GLIDE);
       }
    }
    ```
  - In vanilla Minecraft Java:
    - When fall flying (gliding with Elytra), `fallFlyTicks` counts up continuously while airborne.
    - Every 10 ticks, an `ELYTRA_GLIDE` game event is triggered.
    - Every 20 ticks (1 second of flight), 1 durability is consumed from the equipped Elytra glider (`checkFallFlyTicks % 20 == 0`).
    - If the Elytra's durability falls below minimum (`!can_glide()`), or if the player lands or dies, fall flying state immediately auto-exits (`set_fall_flying(false)`) and `fall_fly_ticks` resets to 0.
  - Previously in Neth:
    - Neth lacked `fall_fly_ticks` tracking and durability degradation entirely; gliding players could fly infinitely without consuming Elytra durability.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `should_damage_elytra` and `should_emit_elytra_game_event` helpers in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `fall_fly_ticks` atomic tracking in `LivingEntity` (`crates/pumpkin-core/src/entity/living.rs`).
  - Added Elytra 20-tick durability damage, condition checks, and auto-exit logic in `LivingEntity::tick`.
  - Added reset of `fall_flying` and `fall_fly_ticks` in `LivingEntity::on_death`.
  - Added unit test `test_elytra_decay_timing_parity` in `combat.rs`. Total 31/31 tests in `entity::combat::tests` passing.

## Gap 146: Firework Rocket Boost during Elytra Gliding, Flight Duration Formula, Launch Sound & Explosion Direct Damage Parity
- **Vanilla Reference**:
  - `FireworkRocketItem.java:28-75`:
    ```java
    @Override
    public InteractionResult useOn(final UseOnContext context) {
       Level level = context.getLevel();
       Player player = context.getPlayer();
       if (player != null && player.isFallFlying()) {
          return InteractionResult.PASS;
       }
       // Spawns FireworkRocketEntity outside block face (+0.15)
       return InteractionResult.SUCCESS;
    }

    @Override
    public InteractionResult use(final Level level, final Player player, final InteractionHand hand) {
       if (player.isFallFlying()) {
          ItemStack itemStack = player.getItemInHand(hand);
          if (level instanceof ServerLevel serverLevel) {
             if (player.dropAllLeashConnections(null)) {
                level.playSound(null, player, SoundEvents.LEAD_BREAK, SoundSource.NEUTRAL, 1.0F, 1.0F);
             }
             Projectile.spawnProjectile(new FireworkRocketEntity(level, itemStack, player), serverLevel, itemStack);
             itemStack.consume(1, player);
             player.awardStat(Stats.ITEM_USED.get(this));
          }
          return InteractionResult.SUCCESS;
       } else {
          return InteractionResult.PASS;
       }
    }
    ```
  - `FireworkRocketEntity.java:62-70, 127-147, 169-172, 223-256`:
    - Lifetime formula:
      ```java
      int flightCount = 1;
      Fireworks fireworks = sourceItemStack.get(DataComponents.FIREWORKS);
      if (fireworks != null) {
         flightCount += fireworks.flightDuration();
      }
      this.lifetime = 10 * flightCount + this.random.nextInt(6) + this.random.nextInt(7);
      ```
      - No fireworks component / duration 0: `flightCount = 1` -> `lifetime = 10 + [0, 11] = 10..=21` ticks.
      - Duration 1 (standard crafted rocket): `flightCount = 1 + 1 = 2` -> `lifetime = 20 + [0, 11] = 20..=31` ticks.
      - Duration 2: `flightCount = 1 + 2 = 3` -> `lifetime = 30 + [0, 11] = 30..=41` ticks.
      - Duration 3: `flightCount = 1 + 3 = 4` -> `lifetime = 40 + [0, 11] = 40..=51` ticks.
    - Attached boost velocity:
      ```java
      movement.add(
         lookAngle.x * 0.1 + (lookAngle.x * 1.5 - movement.x) * 0.5,
         lookAngle.y * 0.1 + (lookAngle.y * 1.5 - movement.y) * 0.5,
         lookAngle.z * 0.1 + (lookAngle.z * 1.5 - movement.z) * 0.5
      );
      ```
    - Launch sound on tick 0 (`life == 0`):
      `SoundEvents.FIREWORK_ROCKET_LAUNCH` (`entity.firework_rocket.launch`) with volume `3.0F`, pitch `1.0F`, category `SoundSource.AMBIENT`.
    - Explosion damage parity:
      - Attached glider receives direct full damage: `5.0F + explosions.size() * 2`.
      - Surrounding entities within 5.0 blocks receive radial falloff damage: `damageAmount * sqrt((5.0 - dist) / 5.0)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `calculate_firework_lifetime`, `calculate_elytra_firework_boost_velocity`, `calculate_firework_direct_damage`, and `calculate_firework_explosion_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `FireworkRocketEntity`:
    - Synchronized `life_time_for` and `new_shot_projectile` with vanilla `flightCount = 1 + duration` formula.
    - Added launch sound emission `Sound::EntityFireworkRocketLaunch` (`volume: 3.0, pitch: 1.0, Ambient`) on tick 0.
    - Applied `calculate_elytra_firework_boost_velocity` during attached flight.
    - Separated direct explosion damage for the attached flyer from radial distance damage for nearby targets.
  - Updated `FireworkRocketItem::use_on_block` to return `BlockActionResult::Pass` while fall flying so players boost instead of placing firework rockets on clicked blocks.
  - Added unit test `test_firework_flight_duration_and_elytra_boost_parity` in `combat.rs` and updated tests in `firework_rocket.rs`. Total 32/32 tests in `entity::combat::tests` passing.

## Gap 147: Player Pose Dimensions, Forced Sneak & Crawling (1-Block Gap) Clearance Parity
- **Vanilla Reference**:
  - `Avatar.java:21-37` and `LivingEntity.java:196`:
    ```java
    STANDING_DIMENSIONS = EntityDimensions.scalable(0.6F, 1.8F).withEyeHeight(1.62F);
    POSES = ImmutableMap.builder()
       .put(Pose.STANDING, STANDING_DIMENSIONS)
       .put(Pose.SLEEPING, SLEEPING_DIMENSIONS) // 0.2F, 0.2F, eye: 0.2F
       .put(Pose.FALL_FLYING, EntityDimensions.scalable(0.6F, 0.6F).withEyeHeight(0.4F))
       .put(Pose.SWIMMING, EntityDimensions.scalable(0.6F, 0.6F).withEyeHeight(0.4F))
       .put(Pose.SPIN_ATTACK, EntityDimensions.scalable(0.6F, 0.6F).withEyeHeight(0.4F))
       .put(Pose.CROUCHING, EntityDimensions.scalable(0.6F, 1.5F).withEyeHeight(1.27F))
       .put(Pose.DYING, EntityDimensions.fixed(0.2F, 0.2F).withEyeHeight(1.62F))
       .build();
    ```
  - `Player.java:345-377`:
    ```java
    protected void updatePlayerPose() {
       if (this.canPlayerFitWithinBlocksAndEntitiesWhen(Pose.SWIMMING)) {
          Pose desiredPose = this.getDesiredPose();
          Pose actualPose;
          if (this.isSpectator() || this.isPassenger() || this.canPlayerFitWithinBlocksAndEntitiesWhen(desiredPose)) {
             actualPose = desiredPose;
          } else if (this.canPlayerFitWithinBlocksAndEntitiesWhen(Pose.CROUCHING)) {
             actualPose = Pose.CROUCHING;
          } else {
             actualPose = Pose.SWIMMING;
          }
          this.setPose(actualPose);
       }
    }
    ```
    - When a player cannot fit in a 1.8m standing space due to an overhead block (e.g. 1.5m clearance), the engine automatically forces `Pose.CROUCHING` (forced sneak).
    - When a player cannot fit in a 1.5m sneaking space due to a low overhead block (e.g. 1.0m clearance with trapdoors or pistons), the engine automatically forces `Pose.SWIMMING` (crawling).
    - If the player is completely suffocated (cannot even fit `Pose.SWIMMING` 0.6m), pose is kept without abrupt reset.
    - When client releases the sneak key (`Action::StopSneaking`), input must be 0 instead of re-sending `SPlayerInput::SNEAK` so the player can actually stand back up.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `get_player_pose_dimensions` and `determine_player_actual_pose` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `Player::update_player_pose` in `crates/pumpkin-core/src/entity/player.rs` to route through `determine_player_actual_pose`.
  - Fixed severe bug in `crates/pumpkin-core/src/net/java/play/player_command.rs` where `Action::StopSneaking` was grouped with `Action::StartSneaking` and was sending `SPlayerInput::SNEAK` instead of `input: 0`.
  - Triggered immediate `player.update_player_pose()` in `crates/pumpkin-core/src/net/java/play/player_input.rs` upon sneak toggles.
  - Added unit test `test_player_pose_dimensions_and_crawling_parity` in `combat.rs`. Total 33/33 tests in `entity::combat::tests` passing.

## Gap 148: Player Swimming Vertical Acceleration Impulse, Look Angle Multiplier & Surface Breach Prevention Parity
- **Vanilla Reference**:
  - `Player.java:1392-1402`:
    ```java
    if (this.isSwimming()) {
       double lookAngleY = this.getLookAngle().y;
       double multiplier = lookAngleY < -0.2 ? 0.085 : 0.06;
       if (lookAngleY <= 0.0
          || this.jumping
          || !this.level().getFluidState(BlockPos.containing(this.getX(), this.getY() + 1.0 - 0.1, this.getZ())).isEmpty()) {
          Vec3 movement = this.getDeltaMovement();
          this.setDeltaMovement(movement.add(0.0, (lookAngleY - movement.y) * multiplier, 0.0));
       }
    }
    ```
    - While swimming, player pitch direction directly drives vertical fluid velocity:
      - Steep downwards diving (`lookAngleY < -0.2`): accelerated by multiplier `0.085`.
      - Shallow diving / swimming up (`lookAngleY >= -0.2`): accelerated by multiplier `0.06`.
      - Vertical impulse formula: `delta_y = (lookAngleY - current_vel_y) * multiplier`.
      - Activation condition: `lookAngleY <= 0.0 || is_jumping || head_submerged`.
      - When player breaches surface (`lookAngleY > 0.0`, space not pressed, and head is above fluid), vertical impulse cuts off to prevent launching into the air.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added `calculate_swimming_vertical_impulse` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Integrated swimming vertical impulse application into `LivingEntity::travel_in_fluid` (`crates/pumpkin-core/src/entity/living.rs`) for swimming players.
  - Added unit test `test_swimming_vertical_impulse_and_multiplier_parity` in `combat.rs`. Total 34/34 tests in `entity::combat::tests` passing.

## Gap 149: Player Underwater Air Supply Depletion, Respiration Enchantment, Turtle Helmet Water Breathing & Drowning Damage Timing Parity
- **Vanilla Reference**:
  - `LivingEntity.java:441-463, 509-511, 591-605`:
    ```java
    if (this.isEyeInFluid(FluidTags.WATER) && !level.getBlockState(BlockPos.containing(this.getX(), this.getEyeY(), this.getZ())).is(Blocks.BUBBLE_COLUMN)) {
       boolean canDrownInWater = !this.canBreatheUnderwater()
          && !MobEffectUtil.hasWaterBreathing(this)
          && (!isPlayer || !((Player)this).getAbilities().invulnerable);
       if (canDrownInWater) {
          this.setAirSupply(this.decreaseAirSupply(this.getAirSupply()));
          if (this.shouldTakeDrowningDamage()) {
             this.setAirSupply(0);
             level.broadcastEntityEvent(this, (byte)67);
             this.hurtServer(level, this.damageSources().drown(), 2.0F);
          }
       } else if (this.getAirSupply() < this.getMaxAirSupply() && MobEffectUtil.shouldEffectsRefillAirsupply(this)) {
          this.setAirSupply(this.increaseAirSupply(this.getAirSupply()));
       }
    } else if (this.getAirSupply() < this.getMaxAirSupply()) {
       this.setAirSupply(this.increaseAirSupply(this.getAirSupply()));
    }
    ```
  - `LivingEntity.java:591-601` & `Enchantments.java:287-295`:
    - Respiration enchantment adds `1.0 * level` to `Attributes.OXYGEN_BONUS`.
    - Air supply depletion formula: `oxygenBonus > 0.0 && this.random.nextDouble() >= 1.0 / (oxygenBonus + 1.0) ? currentSupply : currentSupply - 1`.
    - With Respiration III (`oxygen_bonus = 3.0`), chance of depleting air per tick is `1 / (3 + 1) = 25%` (air lasts 4x longer, ~60s instead of 15s).
  - `LivingEntity.java:509-511`:
    - `shouldTakeDrowningDamage`: `current_air <= -20`.
    - When triggered, `setAirSupply(0)` resets negative air to 0, broadcast event 67 (`EntityStatus::DrownParticles`), and inflicts 2.0 damage. This enforces an exact 20-tick (1-second) interval between drown damage ticks.
  - `MobEffectUtil.java:43-49`:
    - `hasWaterBreathing`: `WATER_BREATHING || CONDUIT_POWER || BREATH_OF_THE_NAUTILUS`.
    - `shouldEffectsRefillAirsupply`: `!BREATH_OF_THE_NAUTILUS || WATER_BREATHING || CONDUIT_POWER`.
  - `Player.java:281-283, 325-339`:
    - When wearing a Turtle Helmet (`Item::TURTLE_HELMET` in `EquipmentSlot::HEAD`) while eyes are outside of water (`!isEyeInFluid(FluidTags.WATER)`), ticks `WATER_BREATHING` effect for 200 ticks (10s, amplifier 0, visible false, showIcon true).
    - When diving underwater, the 10-second water breathing duration runs down before air supply begins depleting.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_respiration_oxygen_bonus`, `should_decrease_air_supply`, `calculate_increased_air_supply`, `should_take_drowning_damage`, `has_water_breathing_effects`, and `should_effects_refill_air_supply` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `LivingEntity::decrease_air_supply` in `crates/pumpkin-core/src/entity/living.rs` to read helmet Respiration enchantment level and combine with `Attributes::OXYGEN_BONUS`. Added `LivingEntity::get_equipment_item`.
  - Updated `BreathManager` in `crates/pumpkin-core/src/entity/breath.rs`:
    - Included `StatusEffect::BREATH_OF_THE_NAUTILUS` in `has_water_breathing`.
    - Added `should_effects_refill_airsupply`.
    - Aligned `tick()` with `LivingEntity.java:441-463` to prevent underwater air refill when only `BREATH_OF_THE_NAUTILUS` is active, and enforce exact 20-tick drowning damage cadence with `set_air(0)`.
  - Added `Player::has_turtle_helmet_equipped` and `Player::turtle_helmet_tick` in `crates/pumpkin-core/src/entity/player.rs`, ticking 200-tick Water Breathing when player is out of water wearing a Turtle Helmet.
  - Added unit test `test_air_supply_respiration_drowning_and_turtle_helmet_parity` in `combat.rs`. Total 35/35 tests in `entity::combat::tests` passing.

## Gap 150: Player Submerged Mining Speed, Aqua Affinity, Mining Fatigue Exponent & Block Break Speed Parity
- **Vanilla Reference**:
  - `Player.java:583-609`:
    ```java
    public float getDestroySpeed(final BlockState state) {
       float speed = this.inventory.getSelectedItem().getDestroySpeed(state);
       if (speed > 1.0F) {
          speed += (float)this.getAttributeValue(Attributes.MINING_EFFICIENCY);
       }
       if (MobEffectUtil.hasDigSpeed(this)) {
          speed *= 1.0F + (MobEffectUtil.getDigSpeedAmplification(this) + 1) * 0.2F;
       }
       if (this.hasEffect(MobEffects.MINING_FATIGUE)) {
          int amplifier = this.getEffect(MobEffects.MINING_FATIGUE).getAmplifier();
          float scale = (float)Math.pow(0.3, amplifier + 1);
          speed *= scale;
       }
       speed *= (float)this.getAttributeValue(Attributes.BLOCK_BREAK_SPEED);
       if (this.isEyeInFluid(FluidTags.WATER)) {
          speed *= (float)this.getAttribute(Attributes.SUBMERGED_MINING_SPEED).getValue();
       }
       if (!this.onGround()) {
          speed /= 5.0F;
       }
       return speed;
    }
    ```
  - `Enchantments.java:299-320`:
    - Aqua Affinity applies `AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL` with value `4.0 * level` to `Attributes.SUBMERGED_MINING_SPEED`.
    - Base `Attributes.SUBMERGED_MINING_SPEED` is `0.2` (80% penalty underwater).
    - With Aqua Affinity I: `0.2 * (1.0 + 4.0) = 1.0` (100% normal speed underwater).
  - `Player.java:595`:
    - Fixed 10x penalty bug in Neth where `Mining Fatigue III` was hardcoded to `0.0027` instead of `0.3^3 = 0.027`, and `Mining Fatigue IV` was `0.00081` instead of `0.3^4 = 0.0081`. Now computes `0.3_f32.powi(amplifier + 1)`.
  - `BlockBehaviour.java:345-352`:
    - Block destroy progress per tick: `player.getDestroySpeed(state) / destroySpeed / (canHarvest ? 30 : 100)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_submerged_mining_speed`, `calculate_mining_fatigue_scale`, `calculate_player_destroy_speed`, and `calculate_block_destroy_progress` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `Player::get_mining_speed` in `crates/pumpkin-core/src/entity/player.rs`:
    - Multiplied with `Attributes::BLOCK_BREAK_SPEED`.
    - Applied submerged mining speed when `is_eye_in_water`, combining base `Attributes::SUBMERGED_MINING_SPEED` with helmet Aqua Affinity enchantment (`0.2 * (1 + 4.0 * level)`).
    - Fixed Mining Fatigue penalty multiplier to vanilla formula `0.3^(amp + 1)`.
  - Added unit test `test_submerged_mining_speed_aqua_affinity_and_destroy_speed_parity` in `combat.rs`. Total 36/36 tests in `entity::combat::tests` passing.

## Gap 151: Wind Charge Eye-Height Throw, 10-Tick Cooldown, Sound Pitch Formula & Block Impact Offset Parity
- **Vanilla Reference**:
  - `WindChargeItem.java:27-54, 73-80`:
    ```java
    Projectile.spawnProjectileFromRotation(
       (source, l, itemStack) -> new WindCharge(player, level, player.position().x(), player.getEyePosition().y(), player.position().z()),
       serverLevel, stack, player, 0.0F, 1.5F, 1.0F
    );
    level.playSound(
       null, player.getX(), player.getY(), player.getZ(),
       SoundEvents.WIND_CHARGE_THROW, SoundSource.NEUTRAL,
       0.5F, 0.4F / (level.getRandom().nextFloat() * 0.4F + 0.8F)
    );
    player.awardStat(Stats.ITEM_USED.get(this));
    stack.consume(1, player);
    ```
  - `Items.java:2233`:
    - `WIND_CHARGE` item registration: `new Item.Properties().useCooldown(0.5F)` (10 ticks cooldown).
  - `AbstractWindCharge.java:103-112`:
    - On block collision, explosion is shifted along face normal by 0.25 block (`face_normal * 0.25`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_wind_charge_throw_pitch`, `calculate_wind_charge_spawn_position`, `calculate_wind_charge_block_explosion_pos`, and `calculate_wind_charge_cooldown_ticks` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `WindChargeItem::normal_use_with_hand` in `crates/pumpkin-core/src/item/items/wind_charge.rs`:
    - Checked `has_item_cooldown` and applied 10-tick (0.5s) cooldown on `minecraft:wind_charge`.
    - Spawned projectile from `(player.x, player.get_eye_y(), player.z)`.
    - Played `Sound::EntityWindChargeThrow` at volume 0.5 and pitch `0.4 / (rand * 0.4 + 0.8)`.
    - Incremented `ItemUsed` statistic for `Item::WIND_CHARGE`.
  - Added unit test `test_wind_charge_throw_pitch_spawn_and_impact_parity` in `combat.rs`. Total 37/37 tests in `entity::combat::tests` passing.

## Gap 152: Thrown Trident Loyalty Return Physics, In-Ground Delay, Void Auto-Recall & Channeling Parity
- **Vanilla Reference**:
  - `ThrownTrident.java:63-97`:
    ```java
    if (this.inGroundTime > 4) {
       this.dealtDamage = true;
    }

    Entity currentOwner = this.getOwner();
    int loyalty = this.entityData.get(ID_LOYALTY);
    if (loyalty > 0 && (this.dealtDamage || this.isNoPhysics()) && currentOwner != null) {
       if (!this.isAcceptibleReturnOwner()) {
          if (this.level() instanceof ServerLevel level && this.pickup == AbstractArrow.Pickup.ALLOWED) {
             this.spawnAtLocation(level, this.getPickupItem(), 0.1F);
          }
          this.discard();
       } else {
          if (!(currentOwner instanceof Player) && this.position().distanceTo(currentOwner.getEyePosition()) < currentOwner.getBbWidth() + 1.0) {
             this.discard();
             return;
          }

          this.setNoPhysics(true);
          Vec3 vec = currentOwner.getEyePosition().subtract(this.position());
          this.setPosRaw(this.getX(), this.getY() + vec.y * 0.015 * loyalty, this.getZ());
          double accel = 0.05 * loyalty;
          this.setDeltaMovement(this.getDeltaMovement().scale(0.95).add(vec.normalize().scale(accel)));
          if (this.clientSideReturnTridentTickCount == 0) {
             this.playSound(SoundEvents.TRIDENT_RETURN, 10.0F, 1.0F);
          }
          this.clientSideReturnTridentTickCount++;
       }
    }
    ```
  - `Enchantments.java:1050-1098`:
    - Channeling on Entity Hit: requires `channeling > 0`, `world.is_thundering()`, and `can_see_sky(victim_pos)`.
    - Channeling on Block Hit: requires `channeling > 0`, `world.is_thundering()`, `can_see_sky(block_pos)`, and **the hit block must belong to `BlockTags.LIGHTNING_RODS`**! Regular blocks do not summon lightning.
    - Sound event: `SoundEvents.TRIDENT_THUNDER` (volume 5.0, pitch 1.0).
  - `Entity.java:605` (`getY() < min_y - 64`):
    - Tridents with Loyalty falling into the void auto-recall to owner instead of being lost permanently.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_trident_loyalty_step`, `should_trident_return_loyalty`, `should_trident_trigger_return_after_in_ground`, `is_acceptable_trident_return_owner`, `should_trident_auto_recall_from_void`, `should_channeling_strike_entity`, and `should_channeling_strike_block` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Fixed loyalty return velocity accumulation bug in Neth: previously `vel.y += delta.y * 0.015 * loyalty` was added into persistent velocity, causing exponential vertical overshoot. Now separated into direct position offset `y_offset` and 0.95 velocity drag with normalized acceleration.
  - Added in-ground return delay parity: tridents only initiate loyalty recall after `in_ground_time > 4` ticks.
  - Added void auto-recall safeguard for tridents falling below `y < min_y - 64.0`.
  - Added `is_lightning_rod_block` check to Channeling on block hit: only lightning rod blocks trigger lightning strikes matching vanilla.
  - Played `Sound::ItemTridentThunder` at volume 5.0 and pitch 1.0 on Channeling strikes.
  - Added unit test `test_thrown_trident_loyalty_physics_void_recall_and_channeling_parity` in `combat.rs`. Total 38/38 tests passing in `entity::combat::tests`.

## Gap 153: Elytra Kinetic Damage, Wall Impact Velocity Threshold & Gliding Aerodynamics Parity
- **Vanilla Reference**:
  - `LivingEntity.java:2614-2623`:
    ```java
    private void handleFallFlyingCollisions(final double moveHorLength, final double newMoveHorLength) {
       if (this.horizontalCollision) {
          double diff = moveHorLength - newMoveHorLength;
          float dmg = (float)(diff * 10.0 - 3.0);
          if (dmg > 0.0F) {
             this.playSound(this.getFallDamageSound((int)dmg), 1.0F, 1.0F);
             this.hurt(this.damageSources().flyIntoWall(), dmg);
          }
       }
    }
    ```
    - Damage formula: `(diff * 10.0 - 3.0)` where `diff = moveHorLength - newMoveHorLength`.
    - Threshold: requires horizontal deceleration `diff > 0.3` blocks/tick (6.0 blocks/sec) to inflict damage.
    - Sound event: `SoundEvents.GENERIC_BIG_FALL` if `dmg > 4.0`, else `SoundEvents.GENERIC_SMALL_FALL` at volume 1.0, pitch 1.0.
    - Damage type: `DamageType::FLY_INTO_WALL`.
  - `LivingEntity.java:3202-3214`:
    - `canGlide()` requires `!onGround() && !isPassenger() && !hasEffect(MobEffects.LEVITATION) && hasGliderItem`.
  - `LivingEntity.java:2567-2570`:
    - `if (this.onClimbable()) { this.travelInAir(input); this.stopFallFlying(); }`
  - `LivingEntity.java:2587-2612`:
    - Gliding aerodynamics:
      - Lift force: `cos(pitch_rad)^2`.
      - Gravity adjustment: `gravity * (-1.0 + lift_force * 0.75)`.
      - Dive speed conversion: `movement.y * -0.1 * lift_force` transferred along look angle.
      - Pull up lift conversion: `horizontal_speed * -sin(pitch_rad) * 0.04` with 3.2 vertical multiplier.
      - Heading turn dampening factor 0.1 towards look direction.
      - Drag friction multipliers: `(0.99, 0.98, 0.99)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_fly_into_wall_damage`, `get_fly_into_wall_sound`, `can_entity_glide`, `should_stop_fall_flying_on_climbable`, and `calculate_fall_flying_movement_step` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Refactored `LivingEntity::handle_fall_flying_collisions` and `LivingEntity::update_fall_flying_movement` in `crates/pumpkin-core/src/entity/living.rs` to use verified vanilla formulas.
  - Added unit test `test_elytra_kinetic_damage_wall_collision_and_aerodynamics_parity` in `combat.rs`. Total 39/39 tests passing in `entity::combat::tests`.

## Gap 154: Thrown Splash Potion Sound Events (1053/1054), Effect Duration Threshold (endsWithin 20) & Fire Extinguishing Parity
- **Vanilla Reference**:
  - `AbstractThrownPotion.java:80-90`:
    ```java
    if (potion.potion().isPresent() && potion.potion().get().value().hasInstantEffects()) {
       level.levelEvent(2007, this.blockPosition(), potion.getColor());
       if (!this.isSilent()) {
          level.levelEvent(1054, this.blockPosition(), 0);
       }
    } else {
       level.levelEvent(2002, this.blockPosition(), potion.getColor());
       if (!this.isSilent()) {
          level.levelEvent(1053, this.blockPosition(), 0);
       }
    }
    ```
    - Splash potion impact sends both particle level event (`2002` or `2007` for instant effects) AND break sound level event (`1053` or `1054` for instant effects).
  - `ThrownSplashPotion.java:54-71`:
    - Distance-based scale: `scale = 1.0 - Math.sqrt(dist) / 4.0` for `dist < 16.0`.
    - Effect duration formula: `duration = (int)(scale * d * durationScale + 0.5)`.
    - Minimum duration gate: `if (!newEffect.endsWithin(20)) { entity.addEffect(newEffect, effectSource); }` (effects scaled to <= 20 ticks / 1 second are dropped and never applied).
  - `AbstractThrownPotion.java:97-113`:
    - When potion is water (`is(PotionTags.EXTINGUISHES_ENTITIES)`):
      `if (extinguishesEntities && entity.isOnFire() && entity.isAlive()) { entity.extinguishFire(); }`
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_splash_potion_scale`, `calculate_scaled_effect_duration`, `should_apply_scaled_effect`, `get_splash_potion_break_sound_event`, `get_splash_potion_particle_event`, and `should_water_potion_extinguish_entity` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `PotionApplicationSource::Splash` and enforced `should_apply_scaled_effect` threshold in `PotionContents::apply_effects_to` in `crates/pumpkin-core/src/item/potion.rs`.
  - Updated `SplashPotionEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/splash_potion.rs` to broadcast both particle event (`2002`/`2007`) and sound event (`1053`/`1054`), and extinguish fire on living entities when splashed by a water potion.
  - Added unit test `test_splash_potion_scale_duration_and_sound_events_parity` in `combat.rs`. Total 40/40 tests passing in `entity::combat::tests`.

## Gap 155: Thrown Lingering Potion Wait Time, Radius Shrinkage Per Tick/Use & AreaEffectCloud Parity
- **Vanilla Reference**:
  - `ThrownLingeringPotion.java:34-53`:
    ```java
    areaEffectCloud.setRadius(3.0F);
    areaEffectCloud.setRadiusOnUse(-0.5F);
    areaEffectCloud.setWaitTime(10);
    areaEffectCloud.setRadiusPerTick(-areaEffectCloud.getRadius() / (float)areaEffectCloud.getDuration());
    areaEffectCloud.applyComponentsFromItemStack(itemStack);
    ```
    - Lingering potion creates an `AreaEffectCloud` with initial radius 3.0 blocks, radius on use -0.5 blocks, wait time 10 ticks (0.5 seconds), default duration 600 ticks (30 seconds), and radius shrinkage per tick `-3.0 / 600.0 = -0.005` blocks/tick.
    - Also emits break sound level event `1053` or `1054` (for instant effects) matching `AbstractThrownPotion.java:80-90`.
  - `AreaEffectCloud.java:190-262`:
    - Cloud expiry lifecycle: `if (this.duration != -1 && this.tickCount - this.waitTime >= this.duration) { this.discard(); }`
    - Minimum radius threshold: if `radius < 0.5F` (either through `radiusPerTick` or `radiusOnUse`), the cloud immediately discards.
    - Effect application cadence: entity search and effect reapplication runs every 5 ticks (`tickCount % 5 == 0`).
    - Horizontal distance check: 2D cylindrical plane `xd * xd + zd * zd <= radius * radius` within cloud height 0.5 blocks.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `get_lingering_potion_initial_duration`, `get_lingering_potion_initial_radius`, `get_lingering_potion_wait_time`, `get_lingering_potion_radius_on_use`, `get_lingering_potion_duration_on_use`, `get_lingering_potion_reapplication_delay`, `calculate_lingering_potion_radius_per_tick`, `should_area_effect_cloud_discard`, and `is_entity_in_area_effect_cloud_radius` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `crates/pumpkin-core/src/entity/projectile/lingering_potion.rs` to broadcast break sound level event `1053`/`1054` and pass vanilla constants to `AreaEffectCloudEntity::create` (`wait_time: 10`, `duration_on_use: 0`).
  - Updated `AreaEffectCloudEntity::tick` in `crates/pumpkin-core/src/entity/area_effect_cloud.rs` to enforce vanilla discard check `age - wait_time >= duration`, radius shrinkage threshold `radius < 0.5`, 5-tick search cadence, and 2D cylindrical distance check.
  - Added unit test `test_lingering_potion_and_area_effect_cloud_parity` in `combat.rs`. Total 41/41 tests passing in `entity::combat::tests`.

## Gap 156: Arrow Punch Knockback Resistance Scaling, Pierce Stuck Count Exemption & Player Hit Notification Parity
- **Vanilla Reference**:
  - `AbstractArrow.java:512-523`:
    ```java
    double knockbackResistance = Math.max(0.0, 1.0 - mob.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE));
    Vec3 movement = this.getDeltaMovement().multiply(1.0, 0.0, 1.0).normalize().scale(knockback * 0.6 * knockbackResistance);
    if (movement.lengthSqr() > 0.0) {
       mob.push(movement.x, 0.1, movement.z);
    }
    ```
    - Arrow punch enchantment knockback impulse scales with victim's `KNOCKBACK_RESISTANCE` (`(1.0 - resistance).max(0.0)`).
  - `AbstractArrow.java:468-470`:
    ```java
    if (!this.level().isClientSide() && this.getPierceLevel() <= 0) {
       mob.setArrowCount(mob.getArrowCount() + 1);
    }
    ```
    - Stuck arrow count is incremented ONLY when arrow does not pierce (`pierce_level <= 0`). Piercing arrows pass through without sticking into the victim.
  - `AbstractArrow.java:478-480`:
    ```java
    if (mob instanceof Player && currentOwner instanceof ServerPlayer ownerPlayer && !this.isSilent() && mob != ownerPlayer) {
       ownerPlayer.connection.send(new ClientboundGameEventPacket(ClientboundGameEventPacket.PLAY_ARROW_HIT_SOUND, 0.0F));
    }
    ```
    - When an arrow hits another player, the shooter receives `ClientboundGameEventPacket.PLAY_ARROW_HIT_SOUND` (`GameEvent::ArrowHitPlayer`) to play the hit confirmation sound.
  - `AbstractArrow.java:434, 452-455, 495, 539`, `SpectralArrow.java:18`:
    - Damage formula `(speed * baseDamage).ceil()` + crit bonus `rng % (damage / 2 + 2)`.
    - Sound pitch formula `1.2F / (rng * 0.2F + 0.9F)`.
    - Spectral arrow duration: 200 ticks (10 seconds).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_arrow_damage`, `calculate_arrow_punch_knockback`, `should_arrow_stick_in_entity`, `calculate_arrow_hit_sound_pitch`, and `get_spectral_arrow_duration` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `ArrowEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/arrow.rs` to scale punch knockback with victim's `KNOCKBACK_RESISTANCE`, increment arrow count only if `should_arrow_stick_in_entity(pierce)`, send `CGameEvent::new(GameEvent::ArrowHitPlayer, 0.0)` to shooter when hitting another player, and use vanilla pitch formula.
  - Added unit test `test_arrow_damage_punch_knockback_and_spectral_duration_parity` in `combat.rs`. Total 42/42 tests passing in `entity::combat::tests`.

## Gap 157: SmallFireball Discard on Hit, Mob Griefing Fire Placement & Dragon Fireball AreaEffectCloud Expansion Parity
- **Vanilla Reference**:
  - `SmallFireball.java:34-60`:
    ```java
    if (!(owner instanceof Mob) || serverLevel.getGameRules().get(GameRules.MOB_GRIEFING)) {
       BlockPos pos = hitResult.getBlockPos().relative(hitResult.getDirection());
       if (this.level().isEmptyBlock(pos)) {
          this.level().setBlockAndUpdate(pos, BaseFireBlock.getState(this.level(), pos));
       }
    }
    ```
    - Small Fireball places fire on block impact only if owner is not a mob or `mobGriefing` is enabled.
    - Projectile is discarded after impact on block or entity.
  - `DragonFireball.java:30-60`:
    ```java
    AreaEffectCloud cloud = new AreaEffectCloud(this.level(), this.getX(), this.getY(), this.getZ());
    cloud.setRadius(3.0F);
    cloud.setDuration(600);
    cloud.setRadiusPerTick((7.0F - cloud.getRadius()) / cloud.getDuration());
    cloud.setPotionDurationScale(0.25F);
    cloud.addEffect(new MobEffectInstance(MobEffects.INSTANT_DAMAGE, 1, 1));
    this.level().levelEvent(2006, this.blockPosition(), this.isSilent() ? -1 : 1);
    this.level().addFreshEntity(cloud);
    this.discard();
    ```
    - Dragon Fireball spawns a breath cloud that expands from radius 3.0 up to 7.0 blocks over 600 ticks (`radiusPerTick = (7.0 - 3.0) / 600 = +0.0066667`).
    - Cloud wait time is 0 (activates immediately), radius on use is 0.0, duration on use is 0.
    - Plays dragon breath sound and particle burst via level event `2006`.
    - Projectile is discarded immediately after onHit.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_dragon_fireball_radius_per_tick`, `can_small_fireball_place_fire`, `get_dragon_fireball_initial_radius`, `get_dragon_fireball_max_radius`, `get_dragon_fireball_duration`, and `get_dragon_breath_level_event` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `AreaEffectCloudEntity::create_full` in `crates/pumpkin-core/src/entity/area_effect_cloud.rs` supporting arbitrary positive or negative `radius_on_tick`.
  - Updated `DragonFireballEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/dragon_fireball.rs` to create an expanding cloud with positive `radius_per_tick` and broadcast level event `2006`.
  - Updated `SmallFireballEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/small_fireball.rs` to check `mob_griefing` game rule when owner is a mob and properly discard the fireball projectile on collision.
  - Added unit test `test_fireball_and_dragon_breath_cloud_parity` in `combat.rs`. Total 43/43 tests passing in `entity::combat::tests`.

## Gap 158: Wither Skull Inertia, Kill Heal Owner, Difficulty Effect Scaling & Discard Parity
- **Vanilla Reference**:
  - `WitherSkull.java:42-44`:
    ```java
    @Override
    protected float getInertia() {
       return this.isDangerous() ? 0.73F : super.getInertia();
    }
    ```
    - Dangerous blue skull has inertia `0.73F` while standard black skull has inertia `0.95F` (`AbstractHurtingProjectile.java:156`).
  - `WitherSkull.java:63-74`:
    ```java
    if (wasHurt) {
       if (var8.isAlive()) {
          EnchantmentHelper.doPostAttackEffects(serverLevel, var8, damageSource);
       } else {
          livingOwner.heal(5.0F);
       }
    }
    ```
    - If a victim is killed by the wither skull attack, the living owner (Wither boss) is healed by 5.0 HP (2.5 hearts).
  - `WitherSkull.java:79-88`:
    - Inflicts Wither II effect based strictly on world difficulty: 10 seconds (200 ticks) on Normal, 40 seconds (800 ticks) on Hard, and 0 on Easy / Peaceful.
  - `WitherSkull.java:94-100`:
    - Discards projectile entity immediately after creating explosion (`this.discard()`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `get_wither_skull_inertia`, `get_wither_skull_effect_duration`, and `get_wither_skull_kill_heal_amount` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `WitherSkullEntity::tick` in `crates/pumpkin-core/src/entity/projectile/wither_skull.rs` to apply vanilla acceleration power 0.1 and inertia (0.73 for dangerous skulls, 0.95 for standard skulls).
  - Updated `WitherSkullEntity::on_hit` to heal living owner by 5.0 HP when a victim is killed, scale wither effect duration strictly by difficulty, and discard projectile upon impact.
  - Added unit test `test_wither_skull_inertia_difficulty_effect_and_heal_parity` in `combat.rs`. Total 44/44 tests passing in `entity::combat::tests`.

## Gap 159: Experience Bottle Discard on Hit, Shulker Bullet Levitation & Llama Spit Damage Parity
- **Vanilla Reference**:
  - `ThrownExperienceBottle.java:31-58`:
    ```java
    @Override
    protected double getDefaultGravity() {
       return 0.07;
    }

    @Override
    protected void onHit(final HitResult hitResult) {
       super.onHit(hitResult);
       if (this.level() instanceof ServerLevel level) {
          level.levelEvent(2002, this.blockPosition(), -13083194);
          if (!this.isSilent()) {
             level.levelEvent(1053, this.blockPosition(), 0);
          }

          int xpCount = 3 + this.random.nextInt(5) + this.random.nextInt(5);
          if (hitResult instanceof BlockHitResult blockHitResult) {
             Vec3 blockNormalHit = blockHitResult.getDirection().getUnitVec3();
             ExperienceOrb.awardWithDirection(level, hitResult.getLocation(), blockNormalHit, xpCount);
          } else {
             ExperienceOrb.awardWithDirection(level, hitResult.getLocation(), this.getDeltaMovement().scale(-1.0), xpCount);
          }

          this.discard();
       }
    }
    ```
    - Default gravity is `0.07`, splash color is `-13083194`, XP count is `3 + rand(5) + rand(5)` (triangular distribution between 3 and 11, avg 7).
    - Plays particle spell splash (event 2002) and sound spell splash (event 1053) unless silent.
    - Explicitly discards entity after spawning XP orbs.
  - `ShulkerBullet.java:179-181, 284-295, 331-336`:
    ```java
    @Override
    public void checkDespawn() {
       if (this.level().getDifficulty() == Difficulty.PEACEFUL) {
          this.discard();
       }
    }

    @Override
    protected void onHitEntity(final EntityHitResult hitResult) {
       super.onHitEntity(hitResult);
       Entity target = hitResult.getEntity();
       Entity owner = this.getOwner();
       LivingEntity livingOwner = owner instanceof LivingEntity livingEntity ? livingEntity : null;
       DamageSource damageSource = this.damageSources().mobProjectile(this, livingOwner);
       boolean wasHurt = target.hurtOrSimulate(damageSource, 4.0F);
       if (wasHurt) {
          if (this.level() instanceof ServerLevel serverLevel) {
             EnchantmentHelper.doPostAttackEffects(serverLevel, target, damageSource);
          }

          if (target instanceof LivingEntity livingTarget) {
             livingTarget.addEffect(new MobEffectInstance(MobEffects.LEVITATION, 200), (Entity)MoreObjects.firstNonNull(owner, this));
          }
       }
    }

    @Override
    public boolean hurtServer(final ServerLevel level, final DamageSource source, final float damage) {
       this.playSound(SoundEvents.SHULKER_BULLET_HURT, 1.0F, 1.0F);
       level.sendParticles(ParticleTypes.CRIT, this.getX(), this.getY(), this.getZ(), 15, 0.2, 0.2, 0.2, 0.0);
       this.destroy();
       return true;
    }
    ```
    - Shulker bullet deals 4.0 damage (`DamageType::MOB_PROJECTILE`) and applies Levitation for 200 ticks (10s, amplifier 0).
    - Despawns immediately on Peaceful difficulty.
    - Emits `SoundEvents.SHULKER_BULLET_HURT` and 15 `ParticleTypes.CRIT` when deflected/hit by an attacker (`hurtServer`), distinct from `SHULKER_BULLET_HIT` and explosion particles on impact.
  - `LlamaSpit.java:38-40, 65-67, 70-79`:
    - Base gravity: `0.06`, air drag: `0.99`.
    - Deals 1.0 damage (`DamageType::SPIT`).
    - Discards on block impact and when submerged in water or touching no air.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_experience_bottle_xp`, `get_experience_bottle_gravity`, `get_experience_bottle_splash_color`, `get_shulker_bullet_damage`, `get_shulker_bullet_levitation_duration`, `should_shulker_bullet_despawn_in_peaceful`, `get_shulker_bullet_deflect_particle_count`, `get_llama_spit_damage`, `get_llama_spit_gravity`, and `get_llama_spit_air_drag` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `ExperienceBottleEntity::on_hit` to ensure `entity.remove()` is called and XP count matches vanilla triangular distribution.
  - Updated `ShulkerBulletEntity`:
    - Added Peaceful difficulty check to despawn bullet on tick start.
    - Updated `damage_with_context` to emit `Sound::EntityShulkerBulletHurt` and 15 `Particle::Crit` upon being hit/deflected.
    - Updated impact damage (4.0) and Levitation duration (200 ticks).
  - Updated `LlamaSpitEntity` to utilize `get_llama_spit_damage`.
  - Added unit test `test_experience_bottle_shulker_bullet_and_llama_spit_parity` in `combat.rs`. Total 45/45 tests passing in `entity::combat::tests`.

## Gap 160: Egg Hatching Probability, Snowball Blaze Damage, Ender Pearl Peaceful Endermite Exemption & Evoker Fangs Timing Parity
- **Vanilla Reference**:
  - `ThrownEgg.java:61-75`:
    ```java
    @Override
    protected void onHitEntity(final EntityHitResult hitResult) {
       super.onHitEntity(hitResult);
       hitResult.getEntity().hurt(this.damageSources().thrown(this, this.getOwner()), 0.0F);
    }

    if (this.random.nextInt(8) == 0) {
       int count = 1;
       if (this.random.nextInt(32) == 0) {
          count = 4;
       }
       for (int i = 0; i < count; i++) {
          Chicken chicken = EntityTypes.CHICKEN.create(this.level(), EntitySpawnReason.TRIGGERED);
          chicken.setAge(-24000);
          ...
       }
    }
    ```
    - 1/8 (12.5%) chance to hatch chicks upon impact.
    - 1/32 chance among hatches to produce 4 chicks simultaneously (1/256 total probability).
    - Inflicts 0.0 damage (`DamageType::THROWN`) to trigger entity hit animations and aggression.
  - `Snowball.java:58-60`:
    ```java
    Entity entity = hitResult.getEntity();
    int damage = entity instanceof Blaze ? 3 : 0;
    entity.hurt(this.damageSources().thrown(this, this.getOwner()), damage);
    ```
    - Deals 3.0 damage to Blazes and 0.0 to all other entities.
  - `ThrownEnderpearl.java:107, 125`:
    ```java
    if (this.random.nextFloat() < 0.05F && level.isSpawningMonsters() && level.getLevelData().getDifficulty() != Difficulty.PEACEFUL) {
       Endermite endermite = EntityTypes.ENDERMITE.create(level, EntitySpawnReason.TRIGGERED);
       ...
    }
    newOwner.hurtServer(player.level(), this.damageSources().enderPearl(), 5.0F);
    ```
    - Endermites only spawn with 5% chance if monsters are allowed to spawn AND world difficulty is not Peaceful.
    - Owner receives exactly 5.0 self damage upon teleportation.
  - `EvokerFangs.java:88-103, 110, 117, 138`:
    - Deals 6.0 damage (`DamageType::MAGIC` / `DamageType::INDIRECT_MAGIC`) when `warmupDelayTicks == -8` in expanded bounding box (+0.2, 0.0, +0.2).
    - Sound pitch is `random.nextFloat() * 0.2F + 0.85F` (0.85 .. 1.05).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_egg_hatch_count`, `get_snowball_damage`, `should_ender_pearl_spawn_endermite`, `get_ender_pearl_self_damage`, `get_evoker_fangs_damage`, `is_evoker_fangs_attack_tick`, and `calculate_evoker_fangs_sound_pitch` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `EggEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/egg.rs` to compute hatch count strictly using `calculate_egg_hatch_count(0..8, 0..32)`.
  - Updated `SnowballEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/snowball.rs` to use `get_snowball_damage`.
  - Updated `EnderPearlEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/ender_pearl.rs` to strictly check Peaceful difficulty exemption before spawning Endermites, and use `get_ender_pearl_self_damage`.
  - Updated `EvokerFangsEntity::tick` in `crates/pumpkin-core/src/entity/projectile/evoker_fangs.rs` to use `is_evoker_fangs_attack_tick`, `get_evoker_fangs_damage`, and apply vanilla sound pitch `calculate_evoker_fangs_sound_pitch`.
  - Added unit test `test_egg_snowball_ender_pearl_and_evoker_fangs_parity` in `combat.rs`. Total 46/46 tests passing in `entity::combat::tests`.

## Gap 161: Large Fireball Mob Griefing Explosion & Eye of Ender Shatter Parity
- **Vanilla Reference**:
  - `LargeFireball.java:18-52`:
    ```java
    @Override
    protected void onHit(final HitResult hitResult) {
       super.onHit(hitResult);
       if (this.level() instanceof ServerLevel serverLevel) {
          boolean grief = serverLevel.getGameRules().get(GameRules.MOB_GRIEFING);
          this.level().explode(this, this.getX(), this.getY(), this.getZ(), this.explosionPower, grief, Level.ExplosionInteraction.MOB);
          this.discard();
       }
    }

    @Override
    protected void onHitEntity(final EntityHitResult hitResult) {
       super.onHitEntity(hitResult);
       if (this.level() instanceof ServerLevel serverLevel) {
          Entity var6 = hitResult.getEntity();
          Entity owner = this.getOwner();
          DamageSource damageSource = this.damageSources().fireball(this, owner);
          var6.hurtServer(serverLevel, damageSource, 6.0F);
          EnchantmentHelper.doPostAttackEffects(serverLevel, var6, damageSource);
       }
    }
    ```
    - Direct entity hit inflicts 6.0 damage (`DamageType::FIREBALL`) and applies post attack enchantment effects.
    - Large fireball explosion checks `mob_griefing` game rule: when disabled, explosion creates no fire and cannot destroy blocks.
  - `EyeOfEnder.java:74-113`:
    ```java
    this.surviveAfterDeath = this.random.nextInt(5) > 0;
    ...
    if (this.life > 80 && !this.level().isClientSide()) {
       this.playSound(SoundEvents.ENDER_EYE_DEATH, 1.0F, 1.0F);
       this.discard();
       if (this.surviveAfterDeath) {
          this.level().addFreshEntity(new ItemEntity(this.level(), this.getX(), this.getY(), this.getZ(), this.getItem()));
       } else {
          this.level().levelEvent(2003, this.blockPosition(), 0);
       }
    }
    ```
    - Eye of Ender has 4-in-5 (80%) chance to survive (`random.nextInt(5) > 0`) and drop as item, 1-in-5 (20%) chance to shatter with level event 2003.
    - Flight duration before death is strictly 80 ticks (4 seconds).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `get_large_fireball_damage`, `can_large_fireball_grief`, `should_eye_of_ender_survive`, `get_eye_of_ender_max_life`, and `get_eye_of_ender_break_level_event` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `FireballEntity::on_hit` in `crates/pumpkin-core/src/entity/projectile/fireball.rs` to query `mob_griefing` game rule before igniting blocks on explosion, and use `get_large_fireball_damage`.
  - Updated `EyeOfEnder` in `crates/pumpkin-core/src/entity/projectile/eye_of_ender.rs` to use `should_eye_of_ender_survive` and `get_eye_of_ender_max_life`.
  - Added unit test `test_large_fireball_and_eye_of_ender_parity` in `combat.rs`. Total 47/47 tests passing in `entity::combat::tests`.

## Gap 162: FoodData Saturation & Starvation Timing, Sprint Hunger Check & Fast Regen Parity
- **Vanilla Reference**: `FoodData.java:11-115`, `FoodConstants.java:3-33`, `ServerGamePacketListenerImpl.java:2322-2329`:
  - `FoodConstants.java`:
    - `saturationByModifier(nutrition, modifier) = nutrition * modifier * 2.0F`.
    - `EXHAUSTION_DROP_RATIO = 4.0F`: Khi exhaustion tích luỹ đạt >= 4.0F, reset trừ 4.0F exhaustion và tiêu hao 1.0F saturation (hoặc 1 food level nếu saturation = 0).
    - `FAST_HEALING_TICK_RATE = 10`, `SLOW_HEALING_TICK_RATE = 80`, `STARVATION_TICK_RATE = 80`.
    - `FAST_HEALING_FOOD_LEVEL = 20`, `SLOW_HEALING_FOOD_LEVEL = 18`.
  - `FoodData.java:107-111`:
    ```java
    public boolean hasEnoughFood() {
       return this.foodLevel > 6;
    }
    ```
    - Người chơi chỉ có thể sprint khi `foodLevel > 6` (trừ khi có `allow_flying` bypass).
    - Khi `foodLevel <= 6` và đang sprint, server tự động hủy sprint của người chơi.
  - `FoodData.java:62-67` (Starvation check):
    ```java
    if (this.tickCounter >= 80) {
       if (this.foodLevel <= 0) {
          if (player.getHealth() > 10.0F || difficulty == Difficulty.HARD || player.getHealth() > 1.0F && difficulty == Difficulty.NORMAL) {
             player.hurtServer(serverLevel, player.damageSources().starve(), 1.0F);
          }
       }
       ...
    }
    ```
    - Ở Difficulty `Easy`: Người chơi chỉ mất máu do đói khi máu > 10.0 HP (5 tim).
    - Ở Difficulty `Normal`: Chỉ mất máu khi máu > 1.0 HP (nửa tim).
    - Ở Difficulty `Hard`: Chết vì đói (mất máu cho tới khi tử vong).
    - Ở Difficulty `Peaceful`: Không mất máu vì đói.
  - Fast Regen vs Normal Regen:
    - Fast Regen (`foodLevel >= 20` và `saturationLevel > 0.0F`): Mỗi 10 ticks hồi `saturationSpent / 6.0F` HP và tăng `saturationSpent = min(saturation, 6.0F)` exhaustion.
    - Normal Regen (`foodLevel >= 18`): Mỗi 80 ticks hồi 1.0F HP và tăng 6.0F exhaustion.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_saturation_gain`, `can_sprint_with_food_level`, and `should_starve_take_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `ServerGamePacketListenerImpl` / `handle_player_command` in `crates/pumpkin-core/src/net/java/play/player_command.rs` to prevent starting sprint when `food_level <= 6` and not flying.
  - Updated `HungerManager::tick` in `crates/pumpkin-core/src/entity/hunger.rs` to stop sprinting when `food_level <= 6`, and strictly follow starvation health limits per vanilla Difficulty (`Easy` > 10.0, `Normal` > 1.0, `Hard` down to 0).
  - Added unit test `test_food_data_saturation_sprint_and_starvation_parity` in `crates/pumpkin-core/src/entity/combat.rs`. All 48/48 tests passing in `entity::combat::tests`.

## Gap 163: Totem Death Protection Execution Ordering, Death Sound Suppression, Water Sensitivity & In-Wall/Cramming Parity
- **Vanilla Reference**: `LivingEntity.java:1255-1267`, `LivingEntity.java:1393-1424`, `LivingEntity.java:3162-3174`, `LivingEntity.java:3223-3235`, `LivingEntity.java:428-439`:
  - `LivingEntity.java:1255-1267` (`hurtServer` sound & death logic):
    ```java
    if (this.isDeadOrDying()) {
       if (!this.checkTotemDeathProtection(source)) {
          if (tookFullDamage) {
             this.makeSound(this.getDeathSound());
             this.playSecondaryHurtSound(source);
          }
          this.die(source);
       }
    } else if (tookFullDamage) {
       this.playHurtSound(source);
       this.playSecondaryHurtSound(source);
    }
    ```
    - Khi nhận sát thương chí mạng (`new_health <= 0`): Vanilla Java kiểm tra kích hoạt Totem of Undying (`checkTotemDeathProtection`) TRƯỚC.
    - Nếu Totem cứu mạng: Sức khỏe được hồi thành 1.0F, toàn bộ hiệu ứng trạng thái bị xoá, nhận Absorption II, Regeneration II, Fire Resistance I, phát sự kiện byte 35 (`ProtectedFromDeath`) và âm thanh `item.totem.use`. TUYỆT ĐỐI KHÔNG PHÁT âm thanh chết (`getDeathSound`) và không phát âm thanh bị thương (`hurtSound`), không gọi `die()`.
    - Chỉ khi Totem KHÔNG cứu (hoặc không có Totem, hoặc sát thương thuộc `#minecraft:bypasses_invulnerability` như void/kill) thì mới phát âm thanh chết và gọi `die()`.
  - `LivingEntity.java:3162-3174` (Sensitive to Water):
    - Các entity: `SnowGolem` (`SnowGolem.java:86`), `Blaze` (`Blaze.java:114`), `Enderman` (`Enderman.java:242`), `Strider` (`Strider.java:374`) nhạy cảm với nước (`isSensitiveToWater() == true`).
    - Khi ở trong nước hoặc mưa (`isInWaterOrRain()`), phải chịu 1.0F sát thương loại `DamageType::DROWN`.
  - `LivingEntity.java:3223-3235` (Entity Cramming):
    - Khi `maxEntityCramming > 0` và số thực thể pushable không phải passenger > `maxEntityCramming - 1`, với tỉ lệ 1-in-4 (`random.nextInt(4) == 0`), thực thể chịu 6.0F sát thương loại `DamageType::CRAMMING`.
  - `LivingEntity.java:428-439` (In-Wall Suffocation & Out-of-Border):
    - `isInWall()`: kiểm tra hộp va chạm co lại 80% bề ngang không rỗng và bị kẹt trong khối gây ngạt (`isSuffocating`), nếu không ngủ thì chịu 1.0F sát thương `DamageType::IN_WALL`.
    - Người chơi ở ngoài WorldBorder (`dist < 0.0`): chịu sát thương `Math.max(1, Mth.floor(-dist * damagePerBlock))` loại out-of-border.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Refactored `damage_with_context` in `crates/pumpkin-core/src/entity/living.rs`: Totem death protection check is evaluated prior to emitting death sounds, completely suppressing spurious death sounds and `on_death` triggers when a Totem saves the entity.
  - Added helper functions `should_totem_prevent_death`, `should_play_death_sound`, `should_play_hurt_sound`, `is_entity_sensitive_to_water`, `should_take_cramming_damage`, `calculate_out_of_border_damage`, and `should_take_in_wall_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added unit test `test_totem_death_protection_water_sensitivity_and_suffocation_parity` in `combat.rs`. All 49/49 tests passing in `entity::combat::tests`.

## Gap 164: Thrown Trident Water Drag & Loyalty Despawn Immunity, Splash Water Extinguish Campfire/Candle & Fishing Weather Parity
- **Vanilla Reference**: `ThrownTrident.java:34, 212-222`, `AbstractThrownPotion.java:50-66, 124-135`, `CampfireBlock.java:97-104`, `FishingHook.java:298-306`:
  - `ThrownTrident.java:220-222`:
    ```java
    @Override
    protected float getWaterInertia() {
       return 0.99F;
    }
    ```
    - Trident là vũ khí chuyên dụng dưới nước, hệ số cản nước (`waterInertia`) là `0.99F` (thay vì `0.6F` như mũi tên bình thường hay `0.9F` như trước đây trong Neth).
  - `ThrownTrident.java:212-217`:
    ```java
    @Override
    protected void tickDespawn() {
       int loyalty = this.entityData.get(ID_LOYALTY);
       if (this.pickup != AbstractArrow.Pickup.ALLOWED || loyalty <= 0) {
          super.tickDespawn();
       }
    }
    ```
    - Khi một cây Trident cắm vào khối và có phù phép `Loyalty > 0` đồng thời người ném cho phép nhặt (`pickup == Pickup.ALLOWED`), cây Trident TUYỆT ĐỐI KHÔNG BAO GIỜ bị biến mất/despawn sau 1200 ticks.
  - `AbstractThrownPotion.java:124-135` & `CampfireBlock.java:97-104`:
    - Khi bình thuốc nước (water splash potion) nổ trong phạm vi va chạm (AABB bán kính 4x2x4):
      - Dập tắt các khối `FireBlock` và `SoulFireBlock` thành `Air`.
      - Dập tắt `CampfireBlock` và `SoulCampfireBlock` đang cháy (`lit == true`), chuyển `lit` thành `false` và phát âm thanh `LevelEvent.SOUND_EXTINGUISH_FIRE` (event id `1009`).
  - `FishingHook.java:298-306`:
    ```java
    int speed = 1;
    BlockPos above = this.blockPosition().above();
    if (this.random.nextFloat() < 0.25F && this.level().isRainingAt(above)) {
       speed++;
    }
    if (this.random.nextFloat() < 0.5F && !this.level().canSeeSky(above)) {
       speed--;
    }
    ```
    - Tốc độ đếm nhịp câu cá (`fishingSpeed`): Cơ bản = 1 tick.
    - Nếu trời mưa tại vị trí ngay trên phao câu (`isRainingAt(above)`): 25% cơ hội tăng tốc độ lên 2 (nhanh gấp đôi khi câu dưới mưa).
    - Nếu vị trí trên phao bị che khuất không thấy trời (`!canSeeSky(above)`): 50% cơ hội giảm tốc độ về 0 (chậm gấp đôi khi câu trong nhà/hang động).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_trident_water_inertia`, `should_trident_despawn`, `calculate_fishing_tick_speed`, and `is_douseable_fire_or_campfire` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `WATER_INERTIA = 0.99` and integrated `should_trident_despawn` into `ThrownTrident::tick` in `crates/pumpkin-core/src/entity/projectile/trident.rs`.
  - Updated `extinguish_fire` in `crates/pumpkin-core/src/entity/projectile/splash_potion.rs` to detect both `BlockId::CAMPFIRE` and `BlockId::SOUL_CAMPFIRE`, extinguish `lit` state to `false`, and broadcast chunk world event `1009` (`SOUND_EXTINGUISH_FIRE`).
  - Added unit test `test_thrown_trident_water_inertia_loyalty_despawn_and_douse_parity` in `combat.rs`. Total 50/50 tests passing in `entity::combat::tests`.

## Gap 165: Fishing Rod Hook Retrieval Durability & Pull Motion Parity, Crossbow Multishot Durability, Pitch & Loading Phase Sound Parity
- **Vanilla Reference**: `FishingHook.java:441-480, 500-506`, `FishingRodItem.java:26-28, 38, 50`, `CrossbowItem.java:167-169, 193-200, 234`, `ProjectileWeaponItem.java:54-71`:
  - `FishingHook.java:441-480`:
    - Khi thu cần câu (`retrieve`):
      - Nếu dính ItemEntity: giảm 3 độ bền (`dmg = 3`), kéo item về phía người chơi.
      - Nếu dính LivingEntity/Player/Mob khác: giảm 5 độ bền (`dmg = 5`), kéo entity về phía người chơi (`pullEntity`).
      - Nếu cắn câu (nibble > 0): sinh vật phẩm loot theo bảng câu cá, bắn loot về phía người chơi với vận tốc `Y = ya * 0.1 + sqrt(sqrt(dist_sq)) * 0.08`, rơi 1-6 kinh nghiệm tại vị trí người chơi, giảm 1 độ bền (`dmg = 1`).
      - Nếu phao câu cắm vào đất (`onGround()`): giảm 2 độ bền (`dmg = 2`).
      - Không trúng gì: giảm 0 độ bền (`dmg = 0`).
  - `FishingRodItem.java:38, 50`:
    - Pitch của âm thanh ném phao (`FISHING_BOBBER_THROW`) và thu cần (`FISHING_BOBBER_RETRIEVE`):
      `0.4F / (random.nextFloat() * 0.4F + 0.8F)`.
  - `ProjectileWeaponItem.java:54-71` & `CrossbowItem.java:167-169`:
    - Độ bền tiêu hao khi bắn nỏ (`getDurabilityUse`):
      - Tên (Arrow): 1 độ bền trên mỗi projectile bắn ra.
      - Pháo hoa (Firework Rocket): 3 độ bền trên mỗi projectile bắn ra.
      - Khi có phù phép Multishot (3 projectiles): Neth trước đây chỉ trừ 1 lần duy nhất; Vanilla Java trừ độ bền cho từng projectile trong vòng lặp bắn (tổng cộng 3 độ bền cho 3 mũi tên, 9 độ bền cho 3 quả pháo hoa).
    - Góc bắn Multishot (`calculate_multishot_spread_angle`):
      - 3 projectiles với spread 10.0°: Mũi giữa (0°), mũi lệch trái (-10°), mũi lệch phải (+10°).
    - Cao độ âm thanh bắn nỏ (`getShotPitch`):
      - Mũi tên chính giữa (`index == 0`): pitch = 1.0F.
      - Mũi tên Multishot lẻ (`index & 1 == 1`): `1.0F / (random * 0.5F + 1.8F) + 0.63F`.
      - Mũi tên Multishot chẵn (`index & 1 == 0`): `1.0F / (random * 0.5F + 1.8F) + 0.43F`.
    - Âm thanh kết thúc nạp đạn nỏ (`CROSSBOW_LOADING_END`):
      - Pitch = `1.0F / (random * 0.5F + 1.0F) + 0.2F`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_fishing_rod_damage`, `calculate_fishing_hook_pull_motion`, `calculate_caught_item_velocity`, `calculate_bobber_sound_pitch`, `calculate_crossbow_durability_use`, `calculate_multishot_spread_angle`, `calculate_crossbow_shot_pitch`, and `calculate_crossbow_loading_end_pitch` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `FishingBobberEntity::reel_in` and `catching_fish` in `crates/pumpkin-core/src/entity/projectile/fishing_bobber.rs` using parity helpers.
  - Updated `FishingRodItem::bobber_sound_pitch` in `crates/pumpkin-core/src/item/items/fishing_rod.rs`.
  - Updated `CrossbowItem::fire_projectiles` and `on_stopped_using` in `crates/pumpkin-core/src/item/items/crossbow.rs` to compute total multishot durability consumption and pitch per projectile.
  - Updated `ProjectileWeaponItem::shoot_projectiles` in `crates/pumpkin-core/src/item/items/projectile_weapon.rs` to use `calculate_multishot_spread_angle`.
  - Added unit test `test_fishing_rod_and_crossbow_multishot_parity` in `combat.rs`. Total 51/51 tests passing in `entity::combat::tests`.

## Gap 166: Bow Power Curve & Release Threshold, Firework Horizontal Acceleration Collision Parity, Mending Recursive Chain Repair & Thorns Damage Parity
- **Vanilla Reference**: `BowItem.java:34-35, 52, 74-82`, `FireworkRocketEntity.java:151-154`, `ExperienceOrb.java:296-314`, `Enchantments.java:540-565`:
  - `BowItem.java:74-82`:
    ```java
    public static float getPowerForTime(final int timeHeld) {
       float pow = timeHeld / 20.0F;
       pow = (pow * pow + pow * 2.0F) / 3.0F;
       if (pow > 1.0F) {
          pow = 1.0F;
       }
       return pow;
    }
    ```
    - Đường cong năng lượng bắn cung (`getPowerForTime`): Tăng phi tuyến tính đạt tối đa 1.0F sau 20 ticks.
    - Ngưỡng nhả cung (`releaseUsing`): `if (pow < 0.1) return false;`. Trước đây Neth hardcode `use_ticks < 3`; nay đồng bộ chính xác theo `can_bow_release(power)` (`pow >= 0.1`).
    - Cao độ âm thanh bắn cung: `1.0F / (random.nextFloat() * 0.4F + 1.2F) + pow * 0.5F`.
  - `FireworkRocketEntity.java:151-154`:
    ```java
    if (!this.isShotAtAngle()) {
       double horizontalAcceleration = this.horizontalCollision ? 1.0 : 1.15;
       this.setDeltaMovement(this.getDeltaMovement().multiply(horizontalAcceleration, 1.0, horizontalAcceleration).add(0.0, 0.04, 0.0));
    }
    ```
    - Khi pháo hoa đang bay thẳng lên, hệ số gia tốc phương ngang là 1.15 khi tự do, nhưng khi chạm tường (`horizontalCollision == true`), hệ số gia tốc là 1.0 (triệt tiêu gia tốc đẩy vào tường). Trước đây Neth luôn nhân 1.15 bất kể va chạm.
  - `ExperienceOrb.java:296-314` (Mending Recursive Chain Repair):
    ```java
    private int repairPlayerItems(final ServerPlayer player, final int amount) {
       Optional<EnchantedItemInUse> selected = EnchantmentHelper.getRandomItemWith(EnchantmentEffectComponents.REPAIR_WITH_XP, player, ItemStack::isDamaged);
       if (selected.isPresent()) {
          ...
          int repair = Math.min(toRepairFromXpAmount, itemStack.getDamageValue());
          itemStack.setDamageValue(itemStack.getDamageValue() - repair);
          if (repair > 0) {
             int remaining = amount - repair * amount / toRepairFromXpAmount;
             if (remaining > 0) {
                return this.repairPlayerItems(player, remaining);
             }
          }
          return 0;
       } else {
          return amount;
       }
    }
    ```
    - Khi nhặt quả cầu kinh nghiệm lớn (XP orb), nếu món đồ Mending đầu tiên được sửa đầy mà vẫn còn dư XP (`remaining > 0`), Vanilla tiếp tục sửa đệ quy các món trang bị Mending bị hư hại khác của người chơi cho đến khi hết đồ hư hại hoặc hết XP. Trước đây Neth chỉ sửa 1 món rồi trả XP dư về thanh kinh nghiệm.
  - `Enchantments.java:540-565` (Thorns):
    - Xác suất kích hoạt: `level * 0.15F` (15% mỗi cấp, tối đa 45% ở Thorns III).
    - Sát thương phản hồi: `1.0..=4.0` HP (hoặc `level - 10` nếu level > 10).
    - Món giáp kích hoạt Thorns chịu thêm 2 điểm tiêu hao độ bền (`ChangeItemDamage(2.0F)`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_bow_power_for_time`, `can_bow_release`, `calculate_bow_sound_pitch`, `calculate_firework_horizontal_acceleration`, `calculate_mending_repair`, and `calculate_thorns_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `BowItem::release_bow`, `get_power_for_time`, and `shoot` in `crates/pumpkin-core/src/item/items/bow.rs`.
  - Updated `FireworkRocketEntity::tick` in `crates/pumpkin-core/src/entity/projectile/firework_rocket.rs` to query `horizontal_collision`.
  - Updated `Player::apply_mending_from_xp` in `crates/pumpkin-core/src/entity/player.rs` to compute durability/XP via `calculate_mending_repair` and recursively repair remaining damaged items.
  - Added unit test `test_bow_power_firework_mending_and_thorns_parity` in `combat.rs`. Total 52/52 tests passing in `entity::combat::tests`.

## Gap 167: Primed TNT Invulnerability & Explosion Offset, TNT Short Fuse Formula, Lightning Sound Pitch & Weather Fire Spawn Parity
- **Vanilla Reference**: `PrimedTnt.java:135, 183-185, 210-212`, `LightningBolt.java:89, 100, 105-108`:
  - `PrimedTnt.java:210-212`:
    ```java
    @Override
    public final boolean hurtServer(final ServerLevel level, final DamageSource source, final float damage) {
       return false;
    }
    ```
    - Khối thuốc nổ đã kích hoạt (Primed TNT) hoàn toàn miễn nhiễm với mọi loại sát thương bên ngoài (vũ khí cận chiến, cung tên, v.v.). Trước đây Neth chưa ghi đè `damage_with_context`, cho phép sát thương từ ngoài tác động vào TNT entity.
  - `PrimedTnt.java:135`:
    - Điểm nổ thực tế của Primed TNT có toạ độ Y là `this.getY(0.0625)` (tức `pos.y + height * 0.0625`), mô phỏng tâm nổ nằm ngay sát đáy khối thuốc nổ đang nhấp nháy.
  - `PrimedTnt.java:183-185`:
    - Khi TNT kích nổ dây chuyền do nổ khác hoặc lửa, thời gian đếm ngược kíp nổ ngắn ngẫu nhiên: `random.nextInt(Math.max(1, fuse / 4)) + fuse / 8`. Với fuse mặc định 80, dải nổ ngắn từ 10 đến 29 ticks (thay vì đợi đủ 80 ticks).
  - `LightningBolt.java:89, 100, 105-108`:
    - Âm thanh sét đánh:
      - Tiếng sấm gầm xa (`LIGHTNING_BOLT_THUNDER`): Volume 10000.0, pitch `0.8F + random * 0.2F`.
      - Tiếng sét đánh trúng gần (`LIGHTNING_BOLT_IMPACT`): Volume 2.0, pitch `0.5F + random * 0.2F`.
    - Điều kiện tạo lửa của sét đánh:
      - Chỉ tạo lửa ngẫu nhiên xung quanh khi độ khó là `Difficulty::Normal` hoặc `Difficulty::Hard`. Ở độ khó `Peaceful` hoặc `Easy`, sét đánh không làm cháy lan xung quanh.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_lightning_thunder_pitch`, `calculate_lightning_impact_pitch`, `can_lightning_spawn_fire`, `calculate_tnt_short_fuse`, `calculate_tnt_explosion_pos_y`, and `should_tnt_take_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `TNTEntity::random_short_fuse`, `tick`, and added `damage_with_context` override in `crates/pumpkin-core/src/entity/tnt.rs`.
  - Updated `LightningBoltEntity::tick` in `crates/pumpkin-core/src/entity/lightning.rs` using lightning sound pitch and difficulty fire spawn helpers.
  - Added unit test `test_tnt_and_lightning_parity` in `combat.rs`. Total 53/53 tests passing in `entity::combat::tests`.

## Gap 168: Mooshroom Thunder Hit Immunity & Sound, Turtle Lightning Death & Bowl, Copper Golem De-oxidation, Pig Conversion Equipment & Boat Bubble Column Parity
- **Vanilla Reference**:
  - `MushroomCow.java:85-92`:
    ```java
    @Override
    public void thunderHit(final ServerLevel level, final LightningBolt lightningBolt) {
       UUID lightningBoltUUID = lightningBolt.getUUID();
       if (!lightningBoltUUID.equals(this.lastLightningBoltUUID)) {
          this.setVariant(this.getVariant() == MushroomCow.Variant.RED ? MushroomCow.Variant.BROWN : MushroomCow.Variant.RED);
          this.lastLightningBoltUUID = lightningBoltUUID;
          this.playSound(SoundEvents.MOOSHROOM_CONVERT, 2.0F, 1.0F);
       }
    }
    ```
    - `MushroomCow` KHÔNG gọi `super.thunderHit` khi bị sét đánh! Mooshroom chỉ chuyển đổi variant (Red <-> Brown), không nhận 5.0 sát thương và không bị bốc cháy 8 giây.
    - Phát âm thanh `MOOSHROOM_CONVERT` với volume 2.0F, pitch 1.0F.
  - `Turtle.java:283-285` & `data/minecraft/loot_table/entities/turtle.json`:
    ```java
    @Override
    public void thunderHit(final ServerLevel level, final LightningBolt lightningBolt) {
       this.hurtServer(level, this.damageSources().lightningBolt(), Float.MAX_VALUE);
    }
    ```
    - Rùa (Turtle) khi bị sét đánh nhận `Float.MAX_VALUE` sát thương từ `DamageType::LIGHTNING_BOLT` (chết ngay lập tức) và kích hoạt loot table rơi 1 cái tô (`minecraft:bowl`). Trước đây Neth chưa ghi đè `mob_on_lightning_strike` cho Turtle khiến nó chỉ nhận 5.0 sát thương và không chết.
  - `CopperGolem.java:459-470`:
    - Khi bị sét đánh, Copper Golem giảm mức độ oxy hóa lùi lại đúng **1 bậc** (`weatherState.previous()`, ví dụ Oxidized -> Weathered -> Exposed -> Unaffected), reset `nextWeatheringTick = -1`, và lọc trùng theo UUID tia sét. Trước đây Neth reset thẳng tuột về `Unaffected` trong 1 lần đánh.
  - `Pig.java:200-205` & `Villager.java:744-758`:
    - Chỉ chuyển hóa thành Zombified Piglin / Witch khi độ khó không phải là Peaceful (`can_lightning_convert_mob`).
    - Zombified Piglin khi được tạo từ lợn bị sét đánh được trang bị thanh kiếm vàng `Item::GOLDEN_SWORD` ở tay chính và thiết lập `persistence_required = true` (không bị despawn).
  - `Entity.java:2941-2954, 2976-2991` & `AbstractBoat.java:298-355`:
    - Động lực học Bubble Column (cột bọt nước) cho Entity:
      - Xoáy nước kéo xuống (`dragDown = true`): `yd = Math.max(-0.9, movement.y - 0.03)` trên mặt nước; `yd = Math.max(-0.3, movement.y - 0.03)` khi chìm bên trong.
      - Cột bọt nước đẩy lên (`dragDown = false`): `yd = Math.min(1.8, movement.y + 0.1)` trên mặt nước; `yd = Math.min(0.7, movement.y + 0.06)` khi chìm bên trong.
    - Cột bọt nước tác động lên thuyền (Boat):
      - Đếm `bubble_time` từ 60 ticks.
      - Khi bộ đếm về 0:
        - Nếu `dragDown`: văng toàn bộ hành khách (`ejectPassengers()`), gửi event `BOAT_SINK` (71), xung lực vận tốc `vel.y -= 0.7`.
        - Nếu đẩy lên: gửi event `BOAT_LAUNCH` (72), xung lực vận tốc `vel.y = 0.6`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_entity_bubble_column_above_y`, `calculate_entity_bubble_column_inside_y`, `calculate_boat_bubble_column_effect_y`, `get_turtle_thunder_hit_damage`, and `can_lightning_convert_mob` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `broadcast_entity_status_id` to `World` in `crates/pumpkin-core/src/world/mod.rs`.
  - Updated `MooshroomEntity::mob_on_lightning_strike` in `crates/pumpkin-core/src/entity/passive/mooshroom.rs` (no damage/fire on convert, sound volume 2.0).
  - Implemented `TurtleEntity::mob_on_lightning_strike` in `crates/pumpkin-core/src/entity/passive/turtle.rs` (`f32::MAX` lightning damage).
  - Updated `CopperGolemEntity::mob_on_lightning_strike` in `crates/pumpkin-core/src/entity/passive/copper_golem.rs` (1 stage de-oxidation, UUID dedup).
  - Updated `PigEntity::mob_on_lightning_strike` in `crates/pumpkin-core/src/entity/passive/pig.rs` (golden sword equipment & persistence).
  - Updated `VillagerEntity::mob_on_lightning_strike` in `crates/pumpkin-core/src/entity/passive/villager/mod.rs`.
  - Implemented bubble column tracking and physics in `BoatEntity` (`crates/pumpkin-core/src/entity/vehicle/boat.rs`).
  - Added unit test `test_bubble_column_and_lightning_conversion_parity` in `combat.rs`. Total 54/54 tests passing in `entity::combat::tests`.

## Gap 169: Iron Golem Attack Fling & Variable Damage, Crackiness Sound & Repair Pitch, Snow Golem Shoot Sound Parity
- **Vanilla Reference**:
  - `IronGolem.java:189-220, 261-277`:
    - `IronGolem.doHurtTarget`:
      - Sát thương ngẫu nhiên: `float damage = (int)attackDamage > 0 ? attackDamage / 2.0F + this.random.nextInt((int)attackDamage) : attackDamage;` (với 14.0 base attack: gây ra từ 7.0 đến 20.0 damage).
      - Hất tung mục tiêu lên không trung: `target.setDeltaMovement(target.getDeltaMovement().add(0.0, 0.4F * (1.0 - knockbackResistance), 0.0))`.
      - Phát âm thanh `SoundEvents.IRON_GOLEM_ATTACK`.
    - `IronGolem.actuallyHurt`:
      - Khi nhận sát thương, nếu `Crackiness` thay đổi (vượt ngưỡng 75%, 50%, 25% max health), phát `SoundEvents.IRON_GOLEM_DAMAGE`.
    - `IronGolem.mobInteract`:
      - Sửa chữa bằng Iron Ingot: hồi 25.0 HP, phát `SoundEvents.IRON_GOLEM_REPAIR` với pitch ngẫu nhiên `1.0F + (random - random) * 0.2F`.
  - `SnowGolem.java:133, 144`:
    - Khi bắn Snowball (`performRangedAttack`), phát âm thanh `SoundEvents.SNOW_GOLEM_SHOOT` với pitch ngẫu nhiên `0.4F / (random * 0.4F + 0.8F)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_iron_golem_attack_damage`, `calculate_iron_golem_fling_velocity_y`, `calculate_iron_golem_repair_pitch`, `calculate_iron_golem_crackiness`, and `calculate_snow_golem_shoot_pitch` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `IronGolemEntity` in `crates/pumpkin-core/src/entity/passive/iron_golem.rs`:
    - Track `crackiness: AtomicCell<u8>` và đồng bộ status khi bị thương / sửa chữa.
    - Added `on_attack` với lực hất tung dọc tỷ lệ nghịch với knockback resistance, biến thiên sát thương, và âm thanh `EntityIronGolemAttack`.
    - Updated `mob_interact` dùng `calculate_iron_golem_repair_pitch` và cập nhật crackiness.
  - Updated `SnowGolemEntity::throw_snowball` trong `crates/pumpkin-core/src/entity/passive/snow_golem.rs` phát âm thanh `Sound::EntitySnowGolemShoot` theo công thức pitch Vanilla.
  - Added unit test `test_iron_golem_and_snow_golem_parity` trong `combat.rs`. Total 55/55 tests passing in `entity::combat::tests`.

## Gap 170: Enderman Projectile Immunity & Passenger Check, Water/Rain Teleport Evasion, Shulker Arrow Deflection & Shulker Bullet Duplication Parity
- **Vanilla Reference**:
  - `EnderMan.java:368-398`:
    - Khi nhận sát thương từ Projectile (`source.is(IS_PROJECTILE)`):
      - Nếu `!this.isPassenger()`: lặp 64 lần thử teleport (`repeatedlyTryToTeleport()`) và hủy bỏ hoàn toàn sát thương (trả về `false`).
      - Nếu đang cưỡi xe/thuyền (`this.isPassenger() == true`): không thể né đòn và không dịch chuyển.
    - Khi nhận sát thương không phải từ LivingEntity (ví dụ sát thương nước/mưa `DamageType::DROWN`, hoặc lửa môi trường):
      - Với xác suất 90% (`this.random.nextInt(10) != 0`), Enderman kích hoạt teleport (`this.teleport()`) né khỏi nguồn sát thương môi trường.
  - `Shulker.java:429-472`:
    - Khi đóng vỏ (`this.isClosed() == true`):
      - Hoàn toàn vô hiệu hóa và làm dội ngược mũi tên bắn tới (`directEntity instanceof AbstractArrow`).
    - Khi bị thương:
      - Nếu máu dưới 50% (`getHealth() < getMaxHealth() * 0.5`) và `random.nextInt(4) == 0`: thử dịch chuyển ngẫu nhiên `teleportSomewhere()`.
      - Khi bị trúng bởi đạn Shulker (`ShulkerBullet`):
        - Nếu Shulker đang mở nắp (`!isClosed()`) và dịch chuyển thành công:
        - Tính số Shulker xung quanh trong bán kính 8 block: `failureChance = (shulkerCount - 1) / 5.0F`.
        - Nếu `rand_float >= failureChance`: sinh ra 1 Shulker con/mới tại vị trí cũ với cùng màu sắc variant.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `should_enderman_evade_damage`, `should_shulker_deflect_arrow`, `should_shulker_teleport_on_low_health`, `calculate_shulker_duplication_failure_chance`, and `can_shulker_duplicate` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `EndermanEntity` in `crates/pumpkin-core/src/entity/mob/enderman.rs`:
    - `pre_damage` incorporates `has_vehicle()` passenger check so riding Endermen cannot dodge projectiles.
    - `on_damage` teleports with 90% chance away from non-living damage (water/rain/fire).
    - `mob_tick` applies environmental water/rain drown damage without attacker.
  - Updated `ShulkerEntity` in `crates/pumpkin-core/src/entity/mob/shulker.rs`:
    - `pre_damage` deflects arrows when closed using `should_shulker_deflect_arrow`.
    - `on_damage` uses `should_shulker_teleport_on_low_health` and spawns duplicated Shulker with matching color on Shulker Bullet hit.
  - Added unit test `test_enderman_and_shulker_parity` in `combat.rs`. Total 56/56 tests passing in `entity::combat::tests`.

## Gap 171: Creeper Fire Charge Ignition & Goat Target Exclusion, Fall Swell & Explosion Radius, Blaze Rain Sensitivity Parity
- **Vanilla Reference**:
  - `Creeper.java:210-228`:
    - Khi tương tác với Creeper, ngoài `FLINT_AND_STEEL`, người chơi có thể dùng `FIRE_CHARGE` (ItemTag `#minecraft:creeper_igniters`) để châm ngòi nổ cưỡng bức (`ignite()`).
    - Khi dùng `FIRE_CHARGE`: phát âm thanh `SoundEvents.FIRECHARGE_USE` (thay vì `FLINTANDSTEEL_USE`), pitch `random.nextFloat() * 0.4F + 0.8F`, tiêu hao 1 Fire Charge stack nếu không ở chế độ Creative.
  - `Creeper.java:154-158`:
    - `setTarget`: Creeper không bao giờ nhắm mục tiêu là Dê (`if (!(target instanceof Goat))`).
  - `Creeper.java:89-92`:
    - Khi rơi từ trên cao (`causeFallDamage`), Creeper tích lũy mức độ sưng nổ: `swell += (int)(fallDistance * 1.5)`, bị giới hạn ở `maxSwell - 5` (25/30 ticks). Khi rơi xuống từ trên cao, Creeper nổ nhanh hơn bình thường rất nhiều.
  - `Creeper.java:232`:
    - Bán kính nổ phóng to gấp 2 lần khi Creeper tích điện (`isPowered()`): `base_radius * 2.0`.
  - `Blaze.java:114` & `LivingEntity.java:3162-3174`:
    - Blaze nhạy cảm với nước (`isSensitiveToWater() == true`).
    - Nhận 1.0 sát thương chết đuối (`DamageType::DROWN`) khi chạm nước hoặc khi đứng dưới trời mưa (`is_raining_at`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `is_creeper_igniter`, `calculate_creeper_ignite_sound_pitch`, `calculate_creeper_explosion_radius`, `calculate_creeper_fall_swell`, `can_creeper_target_entity`, and `should_blaze_take_water_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `CreeperEntity` in `crates/pumpkin-core/src/entity/mob/creeper.rs`:
    - `mob_interact` supports `Item::FIRE_CHARGE` with `Sound::ItemFirechargeUse` and stack consumption.
    - `set_mob_target` ignores `EntityType::GOAT`.
    - `on_damage` uses `calculate_creeper_fall_swell`.
    - `explode` uses `calculate_creeper_explosion_radius`.
  - Updated `BlazeEntity::mob_tick` in `crates/pumpkin-core/src/entity/mob/blaze.rs` to check rain exposure (`is_raining_at`) and apply drown damage.
  - Added unit test `test_creeper_and_blaze_parity` in `combat.rs`. Total 57/57 tests passing in `entity::combat::tests`.

## Gap 172: Guardian & Elder Guardian Thorns Spikes Moving State & Laser Beam Damage Separation Parity
- **Vanilla Reference**:
  - `Guardian.java:311-317`:
    - Khi nhận sát thương:
      ```java
      if (!this.isMoving()
         && !source.is(DamageTypeTags.AVOIDS_GUARDIAN_THORNS)
         && !source.is(DamageTypes.THORNS)
         && source.getDirectEntity() instanceof LivingEntity cause) {
         cause.hurtServer(level, this.damageSources().thorns(this), 2.0F);
      }
      ```
    - Chỉ phản đòn gai nhọn 2.0 Thorns damage khi:
      - Guardian **không di chuyển** (`!this.isMoving()`) - khi bơi gai thu lại, khi dừng gai xòe ra.
      - Nguồn sát thương không mang tag `#minecraft:avoids_guardian_thorns`.
      - Nguồn sát thương không phải là `THORNS` (tránh vòng lặp phản đòn vô hạn).
      - Thực thể tấn công trực tiếp (`directEntity`) là một `LivingEntity` (bắn tên, phóng lao từ xa không bị phản sát thương thorns).
      - Attacker trong phản đòn gai là chính Guardian (không khiến victim tự tấn công mình).
  - `Guardian.java:403-415` (GuardianAttackGoal tick):
    - Khi tia laser tấn công hoàn tất (đủ 80 ticks):
      ```java
      float magicDamage = 1.0F;
      if (this.guardian.level().getDifficulty() == Difficulty.HARD) {
         magicDamage += 2.0F;
      }
      if (this.elder) {
         magicDamage += 2.0F;
      }
      ServerLevel serverLevel = getServerLevel(this.guardian);
      target.hurtServer(serverLevel, this.guardian.damageSources().indirectMagic(this.guardian, this.guardian), magicDamage);
      this.guardian.doHurtTarget(serverLevel, target);
      ```
    - Sát thương chia làm 2 giai đoạn:
      1. Đòn ma thuật gián tiếp (`DamageType::INDIRECT_MAGIC`): 1.0F base, +2.0 Hard difficulty, +2.0 Elder Guardian.
      2. Đòn cận chiến vật lý (`doHurtTarget` / `DamageType::MOB_ATTACK`): 6.0 với Guardian, 8.0 với Elder Guardian, chịu giảm trừ bởi giáp và bùa bảo vệ.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `should_guardian_retaliate_thorns`, `GUARDIAN_THORNS_DAMAGE`, and `calculate_guardian_laser_damage` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `moving: AtomicBool` field and tracked data synchronization (`DATA_ID_MOVING`, `DATA_ID_ATTACK_TARGET`) with `is_moving()` and `set_moving()` methods in both `GuardianEntity` and `ElderGuardianEntity`.
  - Updated `GuardianMoveControl` to set `moving` to `true` while swimming/navigating and `false` when stopped or target reached.
  - Updated `on_damage` in both `GuardianEntity` and `ElderGuardianEntity` to retaliate thorns 2.0 only when spikes are extended (`!is_moving()`), direct attacker is living, and damage type is not thorns or exempt tag, passing Guardian as correct damager.
  - Updated `GuardianAttackGoal` in `crates/pumpkin-core/src/entity/ai/goal/guardian_attack.rs` to separate indirect magic damage from physical melee damage via `try_attack`.
  - Added unit test `test_guardian_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 58/58 tests passing in `entity::combat::tests`.

## Gap 173: Witch Potion Drinking Decisions & Sound Pitch, Ranged Splash Potion Selection & Self-Damage Immunity Parity
- **Vanilla Reference**:
  - `Witch.java:134-173` (aiStep):
    - Khi không uống thuốc, Witch cân nhắc uống một bình Potion:
      - `rand < 0.15 && isEyeInFluid(FluidTags.WATER) && !hasEffect(WATER_BREATHING)` -> Potion::WATER_BREATHING
      - `rand < 0.15 && (isOnFire() || lastDamageSource.is(DamageTypeTags.IS_FIRE)) && !hasEffect(FIRE_RESISTANCE)` -> Potion::FIRE_RESISTANCE
      - `rand < 0.05 && health < maxHealth` -> Potion::HEALING
      - `rand < 0.5 && target != null && !hasEffect(SPEED) && dist_sq > 121.0` -> Potion::SWIFTNESS
    - Âm thanh khi uống potion: `SoundEvents.WITCH_DRINK`, pitch `0.8F + random.nextFloat() * 0.4F`.
    - Khi uống xong: xóa modifier giảm tốc `minecraft:drinking` (-0.25 speed), áp dụng hiệu ứng potion.
  - `Witch.java:209-220` (getDamageAfterMagicAbsorb):
    - Miễn nhiễm hoàn toàn với sát thương do chính mình gây ra:
      `if (damageSource.getEntity() == this) { damage = 0.0F; }`
    - Kháng 85% sát thương phép thuật:
      `if (damageSource.is(DamageTypeTags.WITCH_RESISTANT_TO)) { damage *= 0.15F; }`
  - `Witch.java:223-269` (performRangedAttack):
    - Lựa chọn potion ném:
      - Nếu target là `Raider` đồng minh:
        - `target.getHealth() <= 4.0F` -> Potion::HEALING
        - else -> Potion::REGENERATION
        - Reset target (`setTarget(null)`)
      - Nếu target là kẻ thù:
        - `dist >= 8.0 && !target.hasEffect(SLOWNESS)` -> Potion::SLOWNESS
        - `target.health >= 8.0 && !target.hasEffect(POISON)` -> Potion::POISON
        - `dist <= 3.0 && !target.hasEffect(WEAKNESS) && rand < 0.25` -> Potion::WEAKNESS
        - Mặc định: Potion::HARMING
    - Âm thanh ném splash potion: `SoundEvents.WITCH_THROW`, pitch `0.8F + random.nextFloat() * 0.4F`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_witch_drink_pitch`, `calculate_witch_throw_pitch`, `should_witch_drink_water_breathing`, `should_witch_drink_fire_resistance`, `should_witch_drink_healing`, `should_witch_drink_speed`, `determine_witch_thrown_potion`, and `calculate_witch_damage_received` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Updated `WitchEntity` in `crates/pumpkin-core/src/entity/mob/witch.rs`:
    - `mob_tick`: removed fragile caller downcast, using direct `&self.mob_entity.living_entity`. Integrated `should_witch_drink_...` conditions with submerged water check and drinking sound pitch formula `0.8 + rand * 0.4`.
    - `throw_potion`: integrated `determine_witch_thrown_potion` for complete Vanilla potion priority and throw sound pitch formula `0.8 + rand * 0.4`.
    - `pre_damage`: immune to self-inflicted damage when source entity matches witch id.
  - Added unit test `test_witch_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 59/59 tests passing in `entity::combat::tests`.

## Gap 174: Phantom Attack Scaling, Collision/Damage Abort, Swoop Pitch & Ghast Reflected Fireball Instakill Parity
- **Vanilla Reference**:
  - `Phantom.java:83-94, 194-198`:
    - `setPhantomSize(size)` (0..64):
      - Cập nhật attribute attack damage theo kích thước: `Attributes.ATTACK_DAMAGE.setBaseValue(6 + size)`.
      - Bounding box dimension phóng to: `scale(1.0F + 0.15F * size)`.
  - `Phantom.java:277, 118`:
    - Âm thanh bổ nhào Swoop: `SoundEvents.PHANTOM_SWOOP`, volume `10.0F`, pitch `0.95F + random.nextFloat() * 0.1F`.
    - Âm thanh vỗ cánh Flap: `SoundEvents.PHANTOM_FLAP`, pitch `0.95F + random.nextFloat() * 0.05F`.
  - `Phantom.java:487-495` (PhantomSweepAttackGoal):
    - Đòn cắn trúng đích: chuyển về `AttackPhase.CIRCLE`.
    - `else if (Phantom.this.horizontalCollision || Phantom.this.wasHurtRecently())`: nếu Phantom va vào tường hoặc vừa bị đánh trúng (`hurt_cooldown > 0`), Phantom lập tức hủy đòn swoop và lượn vòng lại (`attackPhase = CIRCLE`).
  - `Ghast.java:76-78, 101-108`:
    - Reflected Fireball:
      ```java
      private static boolean isReflectedFireball(final DamageSource source) {
         return source.getDirectEntity() instanceof LargeFireball && source.getEntity() instanceof Player;
      }
      ```
    - Chỉ gây 1000.0F sát thương khi đạn là Fireball VÀ người phản xạ đòn đánh là `Player`.
    - Các nguồn Fireball khác (Blaze, Ghast khác, hoặc bắn thường) chỉ gây sát thương bình thường.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions `calculate_phantom_attack_damage`, `calculate_phantom_dimension_scale`, `calculate_phantom_swoop_pitch`, `calculate_phantom_flap_pitch`, `should_phantom_abort_swoop_on_collision_or_damage`, `is_ghast_reflected_fireball`, and `calculate_ghast_damage_received` in `crates/pumpkin-core/src/entity/combat.rs`.
  - Added `modify_incoming_damage_with_source` to `Mob` trait and wired it into `Mob::damage_with_context`.
  - Updated `GhastEntity` in `crates/pumpkin-core/src/entity/mob/ghast.rs`:
    - `modify_incoming_damage_with_source`: only deals 1000.0F when direct entity is Fireball AND cause is Player via `is_ghast_reflected_fireball`.
  - Updated `PhantomEntity` in `crates/pumpkin-core/src/entity/mob/phantom.rs`:
    - `set_phantom_size`: calculates attack damage via `calculate_phantom_attack_damage`.
    - `PhantomAttackStrategyGoal::tick`: plays swoop sound with volume 10.0 and formula pitch `calculate_phantom_swoop_pitch`.
    - `PhantomSweepAttackGoal::tick`: aborts swoop on wall collision (`horizontal_collision`) or recent damage (`hurt_cooldown > 0`) via `should_phantom_abort_swoop_on_collision_or_damage`.
  - Added unit test `test_phantom_and_ghast_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 60/60 tests passing in `entity::combat::tests`.

## Gap 175: Slime & MagmaCube Size Scaling, Squish Dynamics, Jump Physics, Contact Damage, Sound Pitch & Experience Parity
- **Vanilla Reference**:
  - `Slime.java:119-125, 49-71`:
    - Attack damage scaling: Tiny Slime (`size <= 1`) không gây sát thương (`canDealDamage() = false`). Slime `size > 1` gây `size` sát thương.
    - Experience reward: `xpReward = actualSize` (size 1: 1 XP, size 2: 2 XP, size 4: 4 XP).
    - Sound variants: phân biệt sound thường và small cho hurt, death, squish, jump.
  - `MagmaCube.java:60-66, 80-99, 102-120`:
    - Attack damage & Armor: Luôn gây sát thương kể cả Tiny (`canDealDamage() = isEffectiveAi()`). Sát thương = `size + 2` (size 1: 3, size 2: 4, size 4: 6). Giáp = `size * 3` (size 1: 3, size 2: 6, size 4: 12).
    - Squish decay: `targetSquish *= 0.9F` (thay vì 0.6F như Slime).
    - Jump boost on ground: `getJumpPower() + size * 0.1F`.
    - Jump in lava: `0.22F + size * 0.05F`.
    - Fire immunity: `isOnFire() = false` và EntityType `.fireImmune()` miễn nhiễm 100% sát thương lửa, dung nham, campfire, magma block.
  - `AbstractCubeMob.java:139, 150, 220-245, 280, 292-295`:
    - Push / Collision damage: khi chạm Player hoặc push vào IronGolem, nếu `canDealDamage()`, tấn công mục tiêu và phát âm thanh `SoundEvents.SLIME_ATTACK` (volume 1.0, pitch `(rand - rand) * 0.2 + 1.0`).
    - Landing squish sound pitch: `((rand - rand) * 0.2 + 1.0) / 0.8`.
    - Jump sound pitch: `((rand - rand) * 0.2 + 1.0) * (if isTiny { 1.4 } else { 0.8 })`.
    - Sound volume: `0.4F * size`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_cube_mob_attack_damage`: Slime tiny 0.0 / size, MagmaCube `size + 2`.
    - `should_cube_mob_deal_contact_damage`: checks effective AI and size restriction.
    - `calculate_cube_mob_armor`: MagmaCube `size * 3`, Slime 0.0.
    - `calculate_cube_mob_xp_reward`: returns `actualSize`.
    - `calculate_cube_mob_squish_decay`: 0.6 for Slime, 0.9 for MagmaCube.
    - `calculate_cube_mob_jump_velocity`: adds `size * 0.1` boost for MagmaCube.
    - `calculate_magma_cube_lava_jump_velocity`: `0.22 + size * 0.05`.
    - `calculate_cube_mob_sound_volume`: `0.4 * size`.
    - `calculate_cube_mob_sound_pitch`: adjuster 1.4 for tiny, 0.8 for normal.
    - `calculate_cube_mob_squish_pitch`: divided by 0.8.
    - `calculate_cube_mob_attack_pitch`: `rand_diff * 0.2 + 1.0`.
    - `is_magma_cube_damage_negated`: negates in_fire, on_fire, lava, hot_floor, campfire.
  - Added `is_magma_cube` and `squish_decay` to `CubeKind` and wired them in `slime.rs` and `magma_cube.rs`.
  - Added `mob_push` hook to `Mob` trait and called it in `EntityBase::push`.
  - Updated `CubeMobEntity` in `crates/pumpkin-core/src/entity/mob/cube_mob.rs`:
    - `mob_player_collision` & `mob_push`: attacks player/iron golem and plays `Sound::EntitySlimeAttack` with formula pitch.
    - `get_base_experience_reward`: drops `actualSize` XP.
    - `modify_incoming_damage_with_source`: negates fire damage for MagmaCube.
    - `mob_tick`: decays target squish with `self.kind.squish_decay` and calculates landing pitch via `calculate_cube_mob_squish_pitch`.
    - `CubeMobMoveControl`: boosts Y-velocity by `size * 0.1` when MagmaCube jumps from ground.
    - `CubeMobFloatGoal`: sets Y-velocity `0.22 + size * 0.05` when MagmaCube touches lava.
  - Added unit test `test_slime_and_magma_cube_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 61/61 tests passing in `entity::combat::tests`.

## Gap 176: Ravager Stun/Roar Dynamics, Vex Weapon Equipment & Charge State, Zoglin Targeting & Damage Parity
- **Vanilla Reference**:
  - `Ravager.java:70, 139-145, 209-222, 224-241, 250-254, 295-300`:
    - Experience reward: `xpReward = 20`.
    - Movement speed state: `0.0` khi immobile (stun, roar, attack); `0.35` khi có attack target; `0.3` khi đi tuần bình thường.
    - Attack state: `on_attack` set `attack_tick = 10`, broadcast status 4 (`EntityStatus::StartAttacking`), phát âm thanh `SoundEvents.RAVAGER_ATTACK`.
    - Shield block reaction (`blockedByItem`): 50% stun 40 ticks (`stunned_tick = 40`), broadcast status 39 (`EntityStatus::RavagerStunned`), phát `SoundEvents.RAVAGER_STUNNED`, defender đẩy ravager lùi; 50% defender nhận `strongKnockback`. Khi hết stun: phát `SoundEvents.RAVAGER_ROAR`, set `roar_tick = 20`.
    - Roar (`roar()` tại tick 10): gây 6.0F sát thương cho tất cả `LivingEntity` trong phạm vi mở rộng 4.0 ngoại trừ nhóm Illagers (`AbstractIllager`). Đẩy văng `strongKnockback` tất cả LivingEntity NGOẠI TRỪ Player (`!(entity instanceof Player)`). Broadcast status 69 (`EntityStatus::RavagerRoared`).
  - `Vex.java:49, 61, 64-66, 169-175, 216-220`:
    - Trang bị mặc định: Vex luôn cầm `Items.IRON_SWORD` ở tay chính `MAIN_HAND` với tỉ lệ rơi đồ bằng 0.0 (`setDropChance(EquipmentSlot.MAINHAND, 0.0F)`).
    - Kinh nghiệm rơi: `xpReward = 3`.
    - Flapping: vỗ cánh mỗi 4 ticks (`tickCount % 4 == 0`).
    - Charging: khi lao vào tấn công, đồng bộ dữ liệu `DATA_FLAGS_ID = 1` và phát âm thanh `SoundEvents.VEX_CHARGE`.
  - `Zoglin.java:69-70, 84, 139, 203-207, 252`:
    - Kinh nghiệm rơi: `xpReward = 5`.
    - Targeting: Zoglin là quái thú tấn công điên cuồng mọi mob sống ngoại trừ chính Zoglin và Creeper (`!target.is(ZOGLIN) && !target.is(CREEPER)`).
    - Attack damage: Người lớn gây 6.0 damage, con non gây 0.5 damage.
    - Sound pitch: Con non có pitch cao hơn (`rand_diff * 0.2 + 1.5` so với `rand_diff * 0.2 + 1.0` của người lớn).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_ravager_target_speed`: immobile 0.0, targeting 0.35, idle 0.3.
    - `calculate_ravager_strong_knockback`: computes 3D velocity impulse `(xd/dd * 4, 0.2, zd/dd * 4)`.
    - `should_ravager_roar_knockback_target`: returns true for non-players, false for players.
    - `is_illager_immune_to_ravager_roar`: filters Pillager, Evoker, Vindicator, Illusioner, Witch, Ravager.
    - `get_ravager_xp_reward`: returns 20.
    - `is_vex_flapping`: `tick % 4 == 0`.
    - `get_vex_xp_reward`: returns 3.
    - `can_zoglin_target_entity`: excludes Zoglin and Creeper.
    - `calculate_zoglin_attack_damage`: adult 6.0, baby 0.5.
    - `calculate_zoglin_sound_pitch`: baby base 1.5, adult base 1.0.
  - Updated `RavagerEntity` in `crates/pumpkin-core/src/entity/mob/ravager.rs`:
    - `get_base_experience_reward`: returns 20 XP.
    - `on_attack`: triggers attack animation status 4 and plays `Sound::EntityRavagerAttack`.
    - `roar`: exempts players from knockback, exempts illagers from damage, broadcasts status 69.
    - `strong_knockback`: uses `calculate_ravager_strong_knockback`.
  - Updated `VexEntity` in `crates/pumpkin-core/src/entity/mob/vex.rs`:
    - Equips Iron Sword in mainhand with 0.0 drop chance.
    - Added `is_charging` state and syncs `DATA_FLAGS_ID`.
    - Plays `Sound::EntityVexCharge` on attack.
    - `get_base_experience_reward`: returns 3 XP.
  - Updated `ZoglinEntity` in `crates/pumpkin-core/src/entity/mob/zoglin.rs`:
    - `ActiveTargetGoal` uses `can_zoglin_target_entity`.
    - `get_base_experience_reward`: returns 5 XP.
    - `set_baby`: sets attack damage via `calculate_zoglin_attack_damage(true)`.
    - `mob_tick`: calculates ambient/angry pitch via `calculate_zoglin_sound_pitch`.
  - Added unit test `test_ravager_vex_and_zoglin_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 62/62 tests passing in `entity::combat::tests`.

## Gap 177: Spider, CaveSpider & Nautilus Poison Immunity, CaveSpider Spawn Parity, and Dolphin Swim With Player & Dolphin's Grace Dynamics
- **Vanilla Reference**:
  - `Spider.java:126-128`, `AbstractNautilus.java:462-464`:
    - Poison Immunity: Cả Spider và Nautilus (cùng CaveSpider kế thừa Spider và ZombieNautilus kế thừa AbstractNautilus) đều override `canBeAffected(MobEffectInstance)` trả về `false` khi `newEffect.is(MobEffects.POISON)`.
    - Tag `MINECRAFT_IGNORES_POISON_AND_REGEN` trong Vanilla chỉ chứa các undead mobs (skeleton variants, zombie variants...), hoàn toàn không chứa spider hay nautilus. Việc trước đó chỉ kiểm tra tag khiến Spider, CaveSpider và Nautilus vô tình bị dính hiệu ứng Poison, vi phạm trực tiếp logic miễn nhiễm của Java edition.
  - `CaveSpider.java:52-54`:
    - CaveSpider override `finalizeSpawn(ServerLevelAccessor, DifficultyInstance, EntitySpawnReason, SpawnGroupData)` trả về trực tiếp `groupData` từ `Mob.finalizeSpawn` mà KHÔNG gọi `Spider.finalizeSpawn`.
    - Trong Java `Spider.finalizeSpawn`, nhện thường có 1% cơ hội sinh ra Skeleton Jockey (`Spider.SpiderEffectsGroupData`), và trên độ khó Hard có `0.1 * difficulty` cơ hội nhận ngẫu nhiên một trong các hiệu ứng Speed, Strength, Regeneration hoặc Invisibility. Cave Spider hoàn toàn không có jockey hay các hiệu ứng ngẫu nhiên này.
  - `Dolphin.java:77, 162, 469-515`:
    - `SWIM_WITH_PLAYER_TARGETING`: `TargetingConditions.forNonCombat().range(10.0).ignoreLineOfSight()`.
    - `DolphinSwimWithPlayerGoal(speedModifier = 4.0)` tại priority 2:
      - `canUse()`: tìm kiếm người chơi gần nhất trong bán kính 10 blocks (`dist_sqr <= 100.0`), người chơi phải đang bơi (`player.isSwimming()`), và người chơi đó không phải là mục tiêu tấn công hiện tại của dolphin (`dolphin.getTarget() != player`).
      - `canContinueToUse()`: người chơi còn sống, đang bơi và khoảng cách `< 256.0` (16 blocks).
      - `start()`: cấp hiệu ứng `DOLPHINS_GRACE` 100 ticks (5 giây) cho người chơi.
      - `stop()`: xóa player reference và dừng navigation.
      - `tick()`: nhìn về phía người chơi (`look_at_with_range`); nếu khoảng cách `< 6.25` (2.5 blocks) thì dừng navigation, ngược lại bơi theo người chơi với tốc độ `4.0`; nếu người chơi đang bơi và tỉ lệ `random.nextInt(6) == 0` thì gia hạn hiệu ứng `DOLPHINS_GRACE` 100 ticks.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `is_spider_or_nautilus_immune_to_poison`: checks if entity is Spider, CaveSpider, Nautilus, or ZombieNautilus and effect is POISON.
    - `should_cave_spider_spawn_jockey_or_effects`: returns `false` adhering to `CaveSpider.java:52`.
    - `should_dolphin_swim_with_player`: validates swimming state, distance (`<= 100.0`), and non-target condition.
    - `should_dolphin_continue_swimming_with_player`: validates swimming state, alive, and distance (`< 256.0`).
    - `calculate_dolphin_grace_duration`: returns 100 ticks (5 seconds).
  - Updated `LivingEntity::can_be_affected` in `crates/pumpkin-core/src/entity/living.rs`:
    - Exempts Spider, CaveSpider, Nautilus, and ZombieNautilus from Poison status effect.
  - Updated `CaveSpiderEntity::finalize_spawn` in `crates/pumpkin-core/src/entity/mob/cave_spider.rs`:
    - Skips jockey spawning and random buff effects on hard difficulty, directly returning base mob spawn group data.
  - Updated `DolphinEntity` in `crates/pumpkin-core/src/entity/passive/dolphin.rs`:
    - Implemented `DolphinSwimWithPlayerGoal` matching `Dolphin.java:469-515`.
    - Added `DolphinSwimWithPlayerGoal` to Dolphin's `goal_selector` at priority 2 with speed modifier 4.0.
  - Added unit test `test_spider_cave_spider_and_dolphin_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 63/63 tests passing in `entity::combat::tests`.

## Gap 178: Squid & GlowSquid Flee Dynamics, Directional Ink Cloud, XP Drops, and Water Mob Leashing Parity
- **Vanilla Reference**:
  - `Squid.java:88-90, 188-203, 240-298`, `GlowSquid.java:34-37, 94-100`, `AgeableWaterCreature.java:33-35, 62`:
    - Squid & GlowSquid Flee Goal (`SquidFleeGoal`):
      - Khi bị tổn thương bởi một thực thể sống (`lastHurtByMob != null`) trong nước ở khoảng cách `< 100.0` (10 blocks):
      - Tính toán vector trốn chạy `fleeTo = squid.pos - attacker.pos`.
      - Tính tốc độ trốn thoát: `avoidSpeed = 3.0`, nếu `length > 5.0` thì giảm dần `avoidSpeed -= (length - 5.0) / 5.0`.
      - Nếu khối phía trên là Air, triệt tiêu thành phần trục Y (`fleeTo.y = 0.0`) để mực không bị đẩy vọt lên cạn.
      - Chuyển động được gán thành `movementVector = fleeTo / 20.0`.
      - Mỗi 10 ticks (khi `fleeTicks % 10 == 5`), phun bọt khí `ParticleTypes.BUBBLE` tại vị trí mực đang bơi trốn.
    - Directional Ink Squirt Cloud:
      - Khi bị thương, Squid và GlowSquid phụt ra đám mây mực gồm 30 hạt mực (`ParticleTypes.SQUID_INK` hoặc `GLOW_SQUID_INK`) phát ra từ phía sau thân mực theo góc quay `pitch` và `yaw` của cơ thể (`rotateVector(new Vec3(0.0, -1.0, 0.0))`).
      - Hạt mực phụt ngược chiều di chuyển với hệ số offset scale `(is_baby ? 0.1 : 0.3) + rand * 2.0` và vận tốc hướng tán xạ `(rand * 0.6 - 0.3, -1.0, rand * 0.6 - 0.3)`.
    - Experience Drop Parity:
      - `AgeableWaterCreature.java:34`: Mực rơi `1 + rand.nextInt(3)` (1 đến 3 XP) khi bị tiêu diệt bởi người chơi.
    - Leashing Parity (`Mob.java:1288`, `AgeableWaterCreature.java:62`, `AmbientCreature.java:13`, `Squid.java:88-90`, `Dolphin.java:376`, `Axolotl.java:369`, `Turtle.java:279`, `Zoglin.java:193`):
      - Mặc định quái vật `MobCategory::MONSTER` không thể bị dắt dây (`canBeLeashed() -> false`), ngoại trừ `Zoglin` (`canBeLeashed() -> true`).
      - Sinh vật môi trường `Bat` không thể bị dắt dây (`canBeLeashed() -> false`).
      - Đa số sinh vật dưới nước (`AgeableWaterCreature`) gồm Rùa biển `Turtle`, các loài cá (`Cod`, `Salmon`, `Pufferfish`, `TropicalFish`, `Tadpole`) không thể bị dắt dây (`canBeLeashed() -> false`).
      - Ngoại lệ đặc biệt: `Squid`, `GlowSquid`, `Dolphin`, và `Axolotl` có thể được dắt dây dắt (`canBeLeashed() -> true`).
      - Dân làng `Villager` và thương nhân lang thang `WanderingTrader` không thể bị dắt dây (`canBeLeashed() -> false`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_squid_flee_movement`: computes flee vector scaled by avoid speed and air bound clamp.
    - `should_squid_flee`: validates water condition, attacker existence, and distance `< 100.0`.
    - `calculate_squid_ink_offset_scale`: computes offset scale based on baby status and random factor.
    - `rotate_squid_vector`: rotates 3D vector by body pitch and yaw angles.
    - `get_squid_xp_reward`: returns `1 + rand(0..3)` XP.
    - `is_mob_leashable`: centralizes leashing permissions across all mob categories, water creatures, and vanilla overrides.
  - Updated `Mob` trait and `interact_mob_item` in `crates/pumpkin-core/src/entity/mob/mod.rs`:
    - Added `can_be_leashed` to `Mob` trait defaulting to `combat::is_mob_leashable(self.get_entity().entity_type)`.
    - Enforced `mob.can_be_leashed()` check in `interact_mob_item` before allowing Lead item leashing.
  - Updated `SquidEntity` & `GlowSquidEntity`:
    - Added `flee_ticks` tracking and integrated `SquidFleeGoal` logic with bubble particles into `mob_tick`.
    - Updated `on_damage` to burst 30 directional ink particles behind the squid's body.
    - Implemented `can_be_leashed` returning `true`.
    - Implemented `get_base_experience_reward` returning 1..=3 XP.
  - Updated `TurtleEntity` and `BatEntity`:
    - Overrode `can_be_leashed` returning `false`.
  - Updated `ZoglinEntity`:
    - Overrode `can_be_leashed` returning `true`.
  - Added unit test `test_squid_glow_squid_turtle_and_leash_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 64/64 tests passing in `entity::combat::tests`.

## Gap 179: Frog Tongue Attack, Small Slime & Magma Cube Consumption, Froglight Drops, and Fall Damage Reduction Parity
- **Vanilla Reference**:
  - `Frog.java:80, 304-306, 315-317`, `ShootTongue.java:21-135`, `FrogAi.java:198-205`, `FrogAttackablesSensor.java:13-35`, loot tables `slime.json`, `magma_cube.json`:
    - **Frog Food & Attack Target Selection**:
      - `Frog.canEat(LivingEntity entity)`: Ếch chỉ có thể ăn các thực thể thuộc tag `#minecraft:frog_food` (`minecraft:slime` và `minecraft:magma_cube`) VỚI ĐIỀU KIỆN kích thước khối `cubeMob.getSize() == 1` (kích thước nhỏ nhất). Các slime hay magma cube cỡ vừa (size 2) hoặc lớn (size 4) và các mob khác không thể bị ếch ăn.
      - `FrogAttackablesSensor`: quét mục tiêu con mồi trong bán kính 10 blocks (`TARGET_DETECTION_DISTANCE = 10.0F`).
    - **Frog Tongue Attack & Catch Dynamics (`ShootTongue`)**:
      - Khi tiếp cận mục tiêu trong khoảng cách `< 1.75` blocks (`EATING_DISTANCE = 1.75F`):
        - Phát âm thanh `SoundEvents.FROG_TONGUE` tại vị trí Ếch (category Neutral, volume 2.0, pitch 1.0).
        - Đặt tư thế của Ếch sang `EntityPose::UsingTongue`.
        - Kéo mục tiêu về phía Ếch: `target.velocity = (frog.pos - target.pos).normalize() * 0.75`.
        - Bắt đầu giai đoạn nuốt mồi (`CatchAnimation`).
      - Sau 6 ticks nuốt mồi (`eatAnimationTimer >= 6`):
        - Phát âm thanh `SoundEvents.FROG_EAT` tại vị trí Ếch (category Neutral, volume 2.0, pitch 1.0).
        - Gây 10.0 sát thương tiêu diệt con mồi.
        - Làm rơi vật phẩm tương ứng tại vị trí của con mồi:
          - Nếu con mồi là Small Slime: rơi 1 `minecraft:slime_ball`.
          - Nếu con mồi là Small Magma Cube: rơi 1 Froglight tương ứng với biến thể của Ếch:
            - Biến thể Temperate (`FrogVariant::Temperate`): rơi 1 `minecraft:ochre_froglight`.
            - Biến thể Warm (`FrogVariant::Warm`): rơi 1 `minecraft:pearlescent_froglight`.
            - Biến thể Cold (`FrogVariant::Cold`): rơi 1 `minecraft:verdant_froglight`.
      - Sau 10 ticks (`eatAnimationTimer >= 10`):
        - Kết thúc hành vi, khôi phục tư thế Ếch về `EntityPose::Standing`, đặt lại `tongue_target_id = -1` và xóa mục tiêu.
    - **Frog Fall Damage Reduction**:
      - `Frog.calculateFallDamage(fallDistance, damageModifier) - 5`: Ếch được giảm trừ 5 điểm sát thương rơi tự do so với các mob thông thường.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `can_frog_eat_entity`: checks if entity is Slime or Magma Cube with `size <= 1`.
    - `get_frog_drop_item`: maps victim type and frog variant to dropped item (`SLIME_BALL`, `OCHRE_FROGLIGHT`, `PEARLESCENT_FROGLIGHT`, `VERDANT_FROGLIGHT`).
    - `calculate_frog_tongue_pull_velocity`: computes vector towards frog with factor 0.75.
    - `should_frog_start_eating`: verifies distance threshold `< 1.75`.
    - `calculate_frog_fall_damage_points`: computes fall damage points with 5-point reduction: `((fall_distance - 3.0) * modifier).ceil() - 5`.
  - Updated `FrogEntity` in `crates/pumpkin-core/src/entity/passive/frog.rs`:
    - Implemented `FrogEatGoal` mirroring vanilla `ShootTongue.java` with 3-phase state machine (`MoveToTarget`, `CatchAnimation`, `EatAnimation`), sound events `EntityFrogTongue` / `EntityFrogEat`, pose `UsingTongue`, victim pull velocity, item drop spawning, and victim destruction.
    - Added `ActiveTargetGoal::predicated` in `target_selector` targeting edible small slimes / magma cubes.
    - Registered `FrogEatGoal` in `goals_selector` at priority 2.
    - Updated `calculate_fall_damage` using `combat::calculate_frog_fall_damage_points`.
  - Added unit test `test_frog_tongue_eat_and_froglight_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 65/65 tests passing in `entity::combat::tests`.

## Gap 180: Tadpole Maturation, Biome-Specific Frog Variant, Zero XP Drop, and Goat Horn Drop & Fall Damage Parity
- **Vanilla Reference**:
  - `Tadpole.java:49, 104-106, 213-247, 254-256`, `FrogVariants.java:26-29`:
    - **Tadpole Maturation (`growUp`)**:
      - Khi `age >= 0` (hoặc đạt đủ 24000 ticks lớn), nòng nọc trưởng thành và chuyển hóa thành Ếch (`FrogEntity`).
      - Biến thể của Ếch phụ thuộc vào Biome mà nòng nọc đang sinh sống:
        - Biome có tag `#minecraft:spawns_warm_variant_frogs`: biến thành Warm variant (`FrogVariant::Warm`).
        - Biome có tag `#minecraft:spawns_cold_variant_frogs`: biến thành Cold variant (`FrogVariant::Cold`).
        - Mặc định: biến thành Temperate variant (`FrogVariant::Temperate`).
      - Khi lớn lên, phát âm thanh `SoundEvents.TADPOLE_GROW_UP` (volume 0.15, pitch 1.0) và loại bỏ thực thể nòng nọc cũ khỏi thế giới.
    - **Zero XP Drop (`shouldDropExperience`)**:
      - `Tadpole.shouldDropExperience() -> false`: Nòng nọc không rơi bất kỳ hạt kinh nghiệm nào khi bị tiêu diệt (`get_base_experience_reward = 0`).
  - `Goat.java:79-84, 124-126, 243-246, 312-341`, `RamTarget.java:96-121`:
    - **Goat Fall Damage Reduction**:
      - `Goat.calculateFallDamage - 10`: Dê được giảm 10 điểm sát thương rơi tự do (tương đương rơi thêm 10 blocks mà không mất máu).
    - **Screaming Goat & Unihorn Spawning Chance**:
      - `GOAT_SCREAMING_CHANCE = 0.02`: 2% xác suất sinh ra dê la hét (Screaming Goat) khi sinh tự nhiên hoặc nhân giống.
      - `UNIHORN_CHANCE = 0.1`: 10% xác suất dê trưởng thành sinh ra chỉ có 1 sừng (`DATA_HAS_LEFT_HORN` hoặc `DATA_HAS_RIGHT_HORN` bị gỡ bỏ).
    - **Goat Horn Drop on Collision (`dropHorn`)**:
      - Khi dê trưởng thành (`!isBaby()`) va chạm với khối thuộc tag `#minecraft:snaps_goat_horn` (ví dụ đá, quặng, log...):
        - Nếu dê còn sừng, rơi ra 1 `minecraft:goat_horn` văng ra với vận tốc ngẫu nhiên (`deltaX = -0.2..0.2`, `deltaY = 0.3..0.7`, `deltaZ = -0.2..0.2`).
        - Cập nhật tracked data của sừng bị rụng thành `false`.
        - Phát âm thanh `SoundEvents.GOAT_HORN_BREAK` tại vị trí của dê.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `determine_frog_variant_from_biome_tags`: determines frog variant from `is_warm` and `is_cold` biome flags.
    - `calculate_goat_fall_damage_points`: computes goat fall damage with 10-point reduction: `((fall_distance * modifier).ceil() - 10).max(0)`.
    - `should_spawn_screaming_goat`: checks 2% chance for screaming goat.
    - `should_spawn_screaming_goat_from_parents`: determines offspring screaming state with parent inheritance or 2% mutation.
    - `should_goat_spawn_unihorn`: checks 10% chance for unihorn on adult goats.
    - `select_goat_horn_to_drop`: chooses left/right horn to drop based on current horn status and random choice.
  - Updated `TadpoleEntity` in `crates/pumpkin-core/src/entity/passive/tadpole.rs`:
    - Implemented `grow_up` converting Tadpole to Frog with biome-specific variant and `EntityTadpoleGrowUp` sound.
    - Overrode `get_base_experience_reward` returning 0.
    - Updated `mob_tick` and `mob_interact` to trigger `grow_up` when `age >= 0`.
  - Updated `GoatEntity` in `crates/pumpkin-core/src/entity/passive/goat.rs`:
    - Initialized screaming goat (2%) and unihorn (10%) upon creation.
    - Implemented `drop_horn` dropping `GOAT_HORN`, updating left/right tracked data, and playing `EntityGoatHornBreak`.
    - Added collision detection in `mob_tick` against `#minecraft:snaps_goat_horn` blocks.
    - Updated `calculate_fall_damage` using `combat::calculate_goat_fall_damage_points`.
  - Added unit test `test_tadpole_and_goat_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 66/66 tests passing in `entity::combat::tests`.

## Gap 181: Allay Fall Damage Immunity & Duplication Parity, Axolotl Breeding Variant Inheritance, Play-Dead Enemy Invisibility & Player Regeneration Stacking Parity
- **Vanilla Reference**:
  - `Allay.java:79, 179-180, 285-290, 297, 305`:
    - **Allay Fall Damage Immunity**:
      - `Allay.checkFallDamage`: Allay là thực thể bay lượn, hoàn toàn miễn nhiễm với sát thương rơi tự do (`calculate_fall_damage = 0`, `adjust_damage_amount = 0.0` khi nhận `DamageType::FALL`).
    - **Allay Duplication Audio, Particles & Tag**:
      - Khi Allay đang nhảy múa (`isDancing()`) và người chơi dùng vật phẩm nhân bản (thuộc tag `#minecraft:duplicates_allays` hoặc Amethyst Shard):
        - Phát âm thanh chuông thạch anh `SoundEvents.AMETHYST_BLOCK_CHIME` (volume 2.0, pitch 1.0) thay vì âm thanh ambient sai lệch.
        - Phát 3 hạt tim (`Particle::Heart`, count = 3: `NUM_OF_DUPLICATION_HEARTS = 3`).
        - Đặt thời gian hồi nhân bản 6000 ticks (5 phút).
      - Âm thanh đưa/nhận vật phẩm `ALLAY_ITEM_GIVEN` và `ALLAY_ITEM_TAKEN`: phát với volume 2.0F.
  - `Axolotl.java:313-315, 346-361, 369-371, 474-476, 490-499`:
    - **Axolotl Breeding Variant Inheritance & Blue Mutation**:
      - Khi ghép đôi 2 con Axolotl:
        - Có 1/1200 xác suất (0.083%) đột biến sinh ra Axolotl biến thể Xanh Dương hiếm (`AxolotlVariant::Blue`).
        - Nếu không đột biến, con nhận 50% biến thể từ cha hoặc 50% biến thể từ mẹ (`rand.nextBoolean() ? parent_a : parent_b`).
    - **Axolotl Play-Dead Enemy Invisibility**:
      - Khi Axolotl đang giả chết (`isPlayingDead()`):
        - Các mob thù địch không thể nhắm mục tiêu hoặc tấn công Axolotl (`can_be_seen_as_enemy() -> false`).
    - **Player Regeneration Stacking & Mining Fatigue Removal**:
      - Khi Axolotl cùng người chơi tiêu diệt mục tiêu:
        - Xóa bỏ hoàn toàn hiệu ứng `StatusEffect::MINING_FATIGUE` khỏi người chơi.
        - Tặng thêm 100 ticks (5 giây) hiệu ứng `REGENERATION I`, cộng dồn trực tiếp vào thời gian Regen hiện tại của người chơi, tối đa 2400 ticks (2 phút).
    - **Leashable**:
      - `Axolotl.canBeLeashed() -> true`: Axolotl có thể được dắt bằng dây buộc (Lead).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_allay_fall_damage_points`: returns 0 for fall damage immunity.
    - `is_allay_duplication_item`: verifies item against `#minecraft:duplicates_allays` or `Item::AMETHYST_SHARD`.
    - `select_axolotl_baby_variant`: handles 1/1200 blue variant mutation and 50/50 parental inheritance.
    - `calculate_axolotl_player_regen_duration`: stacks 100 ticks up to max 2400 ticks (2 minutes).
    - `can_axolotl_be_targeted`: returns false when playing dead.
  - Updated `AllayEntity` in `crates/pumpkin-core/src/entity/passive/allay.rs`:
    - Overrode `calculate_fall_damage` (returning 0) and `adjust_damage_amount` (returning 0.0 on `DamageType::FALL`).
    - Updated duplication logic: checks `combat::is_allay_duplication_item`, spawns 3 heart particles, and plays `Sound::BlockAmethystBlockChime` at volume 2.0.
    - Updated `ItemGiven` and `ItemTaken` sound volumes to 2.0.
  - Updated `AxolotlEntity` in `crates/pumpkin-core/src/entity/passive/axolotl.rs`:
    - Added `can_be_seen_as_enemy` checking `combat::can_axolotl_be_targeted`.
    - Overrode `can_be_leashed` returning `true`.
    - Updated `apply_supporting_effects` to query player's active Regen duration and stack it up to 2400 ticks via `combat::calculate_axolotl_player_regen_duration`.
  - Updated `BreedGoal::breed` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Integrated `combat::select_axolotl_baby_variant` for Axolotl breeding.
    - Integrated `combat::should_spawn_screaming_goat_from_parents` for Goat breeding.
  - Added unit test `test_allay_and_axolotl_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 67/67 tests passing in `entity::combat::tests`.

## Gap 182: Bee Fall Damage Immunity, Drowning in Water & Stinger Count Parity, and Camel Dash Velocity, Passenger Capacity & Dash Ready Audio Parity
- **Vanilla Reference**:
  - `Bee.java:230-247, 365-373, 601-603, 610-611`:
    - **Bee Fall Damage Immunity**:
      - `Bee.checkFallDamage`: Ong bay lượn hoàn toàn miễn nhiễm với sát thương rơi tự do (`calculate_fall_damage = 0`, `adjust_damage_amount = 0.0` khi nhận `DamageType::FALL`).
    - **Bee Underwater Drowning**:
      - `Bee.customServerAiStep`: Khi ong ở dưới nước (`isInWater()`), tăng biến đếm `underWaterTicks++`. Nếu thoát khỏi nước, reset `underWaterTicks = 0`.
      - Khi `underWaterTicks > 20` (quá 1 giây chìm trong nước), mỗi tick nhận 1.0 sát thương chết đuối (`damageSources().drown()`).
    - **Bee Stinger Count Incrementation**:
      - `Bee.doHurtTarget`: Khi ong chích trúng mục tiêu sống (`LivingEntity`), số lượng ngòi cắm vào mục tiêu tăng thêm 1: `livingTarget.setStingerCount(livingTarget.getStingerCount() + 1)`.
  - `Camel.java:305-317, 338-340, 565-567`:
    - **Camel Dash Velocity Momentum Dynamics**:
      - Khi người chơi cưỡi kích hoạt lướt (`Camel.executeRidersJump`):
        - Xung lực ngang (`DASH_HORIZONTAL_MOMENTUM = 22.2222F`): `look_angle_xz * (22.2222 * jump_amount * movement_speed * block_speed_factor)`.
        - Xung lực dọc (`DASH_VERTICAL_MOMENTUM = 1.4285F`): `1.4285 * jump_amount * jump_power`.
        - Đặt `dashCooldown = 55` ticks và đồng bộ trạng thái `isDashing = true`.
    - **Camel Dash Ready Audio & Reset**:
      - Khi đếm lùi `dashCooldown` về 0: phát âm thanh `SoundEvents.CAMEL_DASH_READY` tại vị trí của lạc đà.
      - Khi `isDashing()` và `dashCooldown < 50` và lạc đà tiếp đất (`onGround()`) hoặc chạm chất lỏng (`isInLiquid()`), dừng trạng thái lướt (`setDashing(false)`).
    - **Camel Passenger Capacity & Food Healing**:
      - Sức chứa: Cho phép tối đa 2 người cưỡi (`canAddPassenger -> passengers.size() <= 2`).
      - Hồi phục: Cho ăn `Item::CACTUS` hồi 2.0 HP khi chưa đầy máu (`Camel.handleEating`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_bee_fall_damage_points`: returns 0.
    - `calculate_bee_underwater_damage`: calculates drown damage when underwater ticks exceed 20.
    - `calculate_camel_dash_velocity`: computes exact 3D velocity vectors based on yaw, jump amount, movement speed, jump power, and block speed factor.
    - `can_camel_accept_passenger`: checks passenger count <= 2.
  - Updated `BeeEntity` in `crates/pumpkin-core/src/entity/passive/bee.rs`:
    - Added `under_water_ticks: AtomicI32` counter.
    - Overrode `calculate_fall_damage` (returning 0) and `adjust_damage_amount` (returning 0.0 on `DamageType::FALL`).
    - Updated `mob_tick`: ticks `under_water_ticks` in water and applies drowning damage when ticks > 20; resets to 0 on dry land.
    - Updated `on_attack`: increments target's stinger count via `living.set_stinger_count(living.get_stinger_count() + 1)`.
  - Updated `CamelEntity` in `crates/pumpkin-core/src/entity/passive/camel.rs`:
    - Added `dash_cooldown: AtomicI32` counter.
    - Added `get_dash_cooldown`, `set_dash_cooldown`, and `execute_dash` methods.
    - Updated `mob_tick`: decrements `dash_cooldown`, resets dashing state on ground when cooldown < 50, and plays `Sound::EntityCamelDashReady` when cooldown hits 0.
    - Updated `mob_interact`: heals 2.0 HP on cactus feeding and enforces 2-passenger capacity via `combat::can_camel_accept_passenger`.
  - Added unit test `test_bee_and_camel_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 68/68 tests passing in `entity::combat::tests`.

## Gap 183: Polar Bear Attack Warning Stand & Sound Parity, Water Speed Parity, and Panda Genetics Breeding, Sneeze Slime Ball Drop & Variant Distribution Parity
- **Vanilla Reference**:
  - `PolarBear.java:176-181, 205-207, 239-241, 310-335`:
    - **Polar Bear Attack Warning Stand & Roar**:
      - `PolarBear.PolarBearMeleeAttackGoal`: Khi gấu bắc cực chuẩn bị tấn công mục tiêu (`distanceToSqr < (target.bbWidth + 3.0)^2` và `ticksUntilNextAttack <= 10`):
        - Gấu đứng bằng 2 chân sau (`standing = true`, `setStanding(true)`).
        - Gầm tiếng cảnh báo `SoundEvents.POLAR_BEAR_WARNING` qua `playWarningSound()`, hồi chiêu 40 ticks (`warningSoundTicks = 40`).
      - Khi tấn công trúng (`canPerformAttack`) hoặc mục tiêu chạy xa hoặc dừng goal, gấu hạ 2 chân trước xuống đất (`setStanding(false)`).
    - **Water Swimming Speed (`getWaterSlowDown`)**:
      - `PolarBear.getWaterSlowDown() -> 0.98F`: Gấu bắc cực bơi trong nước nhanh hơn các mob thông thường (chỉ giảm tốc 2% thay vì 20%).
    - **Baby Cub Panic & Alert**:
      - Gấu con bị đánh chỉ hoảng sợ chạy (`PanicGoal` với `DamageTypeTags.PANIC_CAUSES`) và gọi mẹ bảo vệ (`alertOthers()`), không trả thù trực tiếp.
  - `Panda.java:231-255, 446-455, 781-797` & `panda_sneeze.json`:
    - **Panda Random Gene Distribution (`Panda.Gene.getRandom`)**:
      - Phân bố 1/16: 0: Lazy (1/16), 1: Worried (1/16), 2: Playful (1/16), 4: Brown (1/16), 5: Weak (1/16), 6: Aggressive (1/16), còn lại (3, 7..15): Normal (10/16 = 5/8).
    - **Panda Genetics Breeding & 1/32 Mutation (`setGeneFromParents`)**:
      - Khi 2 Panda ghép đôi: con nhận 1 alen ngẫu nhiên từ cha và 1 alen ngẫu nhiên từ mẹ với xác suất 50/50.
      - Có 1/32 xác suất (3.125%) đột biến gen chính (`mainGene`), và 1/32 xác suất đột biến gen ẩn (`hiddenGene`) thành một gen ngẫu nhiên.
    - **Baby Panda Sneeze & 1/700 Slime Ball Drop**:
      - Gấu trúc con có hành vi hắt xì:
        - Gấu trúc yếu (`Weak`): tỷ lệ hắt xì 1/500 mỗi tick.
        - Gấu trúc thường: tỷ lệ hắt xì 1/6000 mỗi tick.
      - Chu kỳ hắt xì: đếm từ 1 đến 20 ticks (`sneezeCounter`):
        - Tick 1: phát âm thanh `SoundEvents.PANDA_PRE_SNEEZE`.
        - Tick > 20: phát âm thanh `SoundEvents.PANDA_SNEEZE`, làm giật mình các gấu trúc trưởng thành xung quanh trong phạm vi 10 block (`jumpFromGround()`).
        - Theo loot table `panda_sneeze.json`: có đúng **1/700 cơ hội (0.143%)** rơi ra 1 `Item::SLIME_BALL` tại vị trí hắt xì!
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_panda_random_gene`: implements exact 1/16 gene distribution matching vanilla.
    - `select_panda_baby_genes`: handles parental allele inheritance and 1/32 mutation rates for both main and hidden genes.
    - `should_panda_sneeze`: evaluates tick chance for weak (1/500) and regular (1/6000) baby pandas.
    - `should_panda_drop_slime_ball`: evaluates 1/700 gift loot chance for slime ball drops.
    - `should_polar_bear_stand_and_warn`: checks `dist_sq < (width + 3.0)^2` and `ticks <= 10`.
  - Updated `PolarBearEntity` in `crates/pumpkin-core/src/entity/passive/polar_bear.rs`:
    - Added `warning_sound_ticks: AtomicI32` counter (40 ticks cooldown).
    - Added `play_warning_sound` playing `Sound::EntityPolarBearWarning`.
    - Updated `mob_tick`: decrements `warning_sound_ticks`, triggers standing and warning roar when target is close and attack is imminent, and lowers hind legs when far.
    - Added `on_attack`: resets standing state to false upon attack execution.
  - Updated `PandaEntity` in `crates/pumpkin-core/src/entity/passive/panda.rs`:
    - Updated `PandaGene::random_gene()` using `combat::calculate_panda_random_gene`.
    - Added `set_gene_from_parents` calling `combat::select_panda_baby_genes`.
    - Updated `mob_tick`: handles sneeze counter progression (1..20), plays pre-sneeze and sneeze audio, scares nearby adult pandas, and spawns `Item::SLIME_BALL` on 1/700 chance.
  - Updated `BreedGoal::breed` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added parental genetics inheritance for Panda breeding via `baby_panda.set_gene_from_parents(panda_a, Some(panda_b))`.
  - Added unit test `test_polar_bear_and_panda_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 69/69 tests passing in `entity::combat::tests`.

## Gap 184: Cat & Ocelot Fall Damage Immunity, Cat Breeding Inheritance & Morning Gift Drop Parity, and Ocelot Despawn & Trust Mechanics Parity
- **Vanilla Reference**:
  - `fall_damage_immune.json` & `Cat.java:235-245, 490-530` & `Ocelot.java:138-140`:
    - **Feline Fall Damage Immunity**:
      - Cả Mèo (`minecraft:cat`) và Mèo gấm (`minecraft:ocelot`) đều nằm trong tag `#minecraft:fall_damage_immune`. Cả hai hoàn toàn miễn nhiễm với sát thương rơi tự do (`calculate_fall_damage = 0`, `adjust_damage_amount = 0.0` khi nhận `DamageType::FALL`).
    - **Cat Kitten Breeding Traits Inheritance**:
      - `Cat.getBreedOffspring`: Khi ghép đôi 2 con mèo:
        - Mèo con nhận 50% biến thể (CatVariant) từ cha hoặc 50% từ mẹ (`random.nextBoolean() ? parent_a : parent_b`).
        - Nếu mèo cha/mẹ đã được thuần hóa (`isTame`):
          - Mèo con tự động được thuần hóa theo chủ của cha/mẹ (`setTame(true, owner)`).
          - Mèo con nhận 50% màu vòng cổ (`CollarColor`) từ cha hoặc 50% từ mẹ.
    - **Cat Morning Gift Delivery**:
      - `Cat.giveMorningGift` & `gameplay/cat_morning_gift.json`:
        - Khi chủ thức dậy sau khi ngủ ít nhất 100 ticks (`sleepTimer >= 100`) và roll xác suất thành công (mặc định 70%):
        - Mèo mang quà buổi sáng đến cho chủ theo loot table gồm 7 vật phẩm (tổng trọng số 62):
          - Thỏ: Rabbit Hide (10/62), Rabbit Foot (10/62)
          - Gà/Chim: Raw Chicken (10/62), Feather (10/62)
          - Quái vật: Rotten Flesh (10/62), String (10/62)
          - Quà hiếm: Phantom Membrane (2/62 = ~3.22%).
    - **Ocelot Despawn Mechanics**:
      - `Ocelot.removeWhenFarAway`: Mèo gấm hoang dã chưa tin tưởng người chơi (`!isTrusting()`) sẽ tự động biến mất (despawn) sau 2400 ticks (2 phút) khi ở xa người chơi. Mèo gấm đã tin tưởng (`isTrusting() == true`) sẽ không bao giờ tự động biến mất.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_feline_fall_damage_points`: returns 0 fall damage for cats and ocelots.
    - `select_cat_baby_traits`: computes 50/50 parental variant and collar color inheritance.
    - `select_cat_morning_gift`: selects exact items according to vanilla weighted distribution (including 2/62 Phantom Membrane).
    - `should_cat_give_morning_gift`: evaluates `sleep_timer >= 100` and roll check against gift chance.
    - `should_ocelot_despawn_when_far`: checks `!is_trusting && age_ticks > 2400`.
  - Updated `CatEntity` in `crates/pumpkin-core/src/entity/passive/cat.rs`:
    - Overrode `calculate_fall_damage` (returning 0) and `adjust_damage_amount` (returning 0.0 on `DamageType::FALL`).
    - Added `give_morning_gift` spawning morning gift items from `combat::select_cat_morning_gift`.
  - Updated `OcelotEntity` in `crates/pumpkin-core/src/entity/passive/ocelot.rs`:
    - Overrode `calculate_fall_damage` (returning 0) and `adjust_damage_amount` (returning 0.0 on `DamageType::FALL`).
    - Overrode `remove_when_far_away` using `combat::should_ocelot_despawn_when_far`.
  - Updated `BreedGoal::breed` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added Cat breeding inheritance for kitten variant, taming ownership, and collar color via `combat::select_cat_baby_traits`.
  - Added unit test `test_cat_and_ocelot_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 70/70 tests passing in `entity::combat::tests`.

## Gap 185: Wolf Food Nutrition Healing Scaling, Wolf & Fox Breeding Variant Inheritance, and Fox Safe Fall Distance Attribute Parity
- **Vanilla Reference**:
  - `Wolf.java:mobInteract, 271-285` & `Fox.java:createAttributes, 231-236`:
    - **Wolf Food Nutrition Healing Scaling**:
      - `Wolf.mobInteract`: Khi sói bị thương nhận thức ăn (`isFood(item)`), lượng máu hồi phục tỷ lệ thuận với điểm dinh dưỡng (`nutrition`) của thực phẩm:
        `float f = foodProperties != null ? 2.0F * (float)foodProperties.nutrition() : 1.0F; this.heal(f);`.
        Trước đây Neth hardcode hồi cố định `2.0` máu bất kể loại thức ăn (ví dụ Cooked Beef với nutrition 8 phải hồi `16.0 HP`, Rotten Flesh với nutrition 4 phải hồi `8.0 HP`).
    - **Wolf Breeding Traits & Ownership Inheritance**:
      - `Wolf.getBreedOffspring`: Sói con sinh ra nhận 50% biến thể lông (`WolfVariant`) từ cha hoặc mẹ (`random.nextBoolean() ? parent_a : parent_b`).
      - Nếu sói cha/mẹ đã được thuần hóa (`isTame`):
        - Sói con tự động thuần hóa theo chủ (`setOwnerUUID(this.getOwnerUUID())`).
        - Sói con nhận 50% màu vòng cổ (`CollarColor`) từ cha hoặc mẹ.
    - **Fox Breeding Variant Inheritance**:
      - `Fox.getBreedOffspring`: Cáo con sinh ra nhận 50% biến thể lông (`FoxVariant::Red` hoặc `FoxVariant::Snow`) từ cha hoặc mẹ (`random.nextBoolean() ? parent_a : parent_b`).
    - **Fox Safe Fall Distance Attribute**:
      - `Fox.createAttributes`: Cáo có `Attributes.SAFE_FALL_DISTANCE = 5.0` (thay vì 3.0 blocks như mob thông thường), cho phép rơi tự do từ độ cao 5 block mà không chịu sát thương rơi.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `calculate_wolf_food_healing(nutrition)`: computes `2.0 * nutrition` (fallback 1.0 if no food properties).
    - `select_wolf_baby_traits`: computes 50/50 parental variant and collar color inheritance.
    - `select_fox_baby_variant`: computes 50/50 parental variant (`Red` vs `Snow`) inheritance.
    - `get_fox_safe_fall_distance`: returns `5.0`.
  - Updated `WolfEntity` in `crates/pumpkin-core/src/entity/passive/wolf.rs`:
    - In `mob_interact`: reads `FoodImpl` data component from `item_stack` and heals by `combat::calculate_wolf_food_healing(nutrition)`.
    - Added `WolfEntity::get_variant` returning `WolfVariant`.
  - Updated `FoxEntity` in `crates/pumpkin-core/src/entity/passive/fox.rs`:
    - Sets base attribute `Attributes::SAFE_FALL_DISTANCE` using `combat::get_fox_safe_fall_distance()`.
  - Updated `BreedGoal::breed` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added Wolf breeding inheritance for puppy variant, owner taming, and collar color via `combat::select_wolf_baby_traits`.
    - Added Fox breeding inheritance for kit variant via `combat::select_fox_baby_variant`.
  - Added unit test `test_wolf_and_fox_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 71/71 tests passing in `entity::combat::tests`.

## Gap 186: Equine Eating Nutrition, Baby Growth & Temper Mechanics, and Horse/Donkey Hybrid Mule Breeding Parity
- **Vanilla Reference**:
  - `AbstractHorse.java:handleEating, mobInteract, createOffspringAttribute` & `Horse.java:canMate, getBreedOffspring` & `Donkey.java:canMate, getBreedOffspring` & `Mule.java:canMate`:
    - **Equine Eating Properties & Effects**:
      - `AbstractHorse.handleEating`: Thức ăn cho ngựa/lừa/la có các tác động cụ thể:
        - `Wheat`: hồi 2.0 HP, giảm thời gian lớn 20 giây (400 ticks), tăng điểm thuần hóa (`temper`) +3.
        - `Sugar`: hồi 1.0 HP, giảm thời gian lớn 30 giây (600 ticks), tăng điểm thuần hóa +3.
        - `Hay Block`: hồi 20.0 HP, giảm thời gian lớn 180 giây (3600 ticks), không tăng điểm thuần hóa.
        - `Apple` & `Carrot`: hồi 3.0 HP, giảm thời gian lớn 60 giây (1200 ticks), tăng điểm thuần hóa +3.
        - `Golden Carrot`: hồi 4.0 HP, giảm thời gian lớn 60 giây, tăng điểm thuần hóa +5, kích hoạt sinh sản (`setInLove(player)`) nếu đã thuần hóa và là con trưởng thành.
        - `Golden Apple` & `Enchanted Golden Apple`: hồi 10.0 HP, giảm thời gian lớn 240 giây (4800 ticks), tăng điểm thuần hóa +10, kích hoạt sinh sản (`setInLove(player)`) nếu đã thuần hóa và là con trưởng thành.
      - Trước đây Neth gọi thẳng `animal_interact`, khiến lúa mì và đường cũng kích hoạt `in_love`, không hồi máu theo đúng bảng dinh dưỡng, không đẩy nhanh quá trình lớn cho con non, và khiến con la vô sinh cũng vào chế độ in_love.
    - **Equine Cross-Breeding Compatibility & Hybrid Mule**:
      - `Horse.canMate` & `Donkey.canMate`:
        - Ngựa có thể ghép đôi với Ngựa hoặc Lừa.
        - Lừa có thể ghép đôi với Lừa hoặc Ngựa.
        - La (`Mule`) là con lai vô sinh (`canMate` trả về false).
        - Trước đây trong Neth, `BreedGoal::find_mate` lọc theo `c_entity.entity_type != my_type`, ngăn chặn hoàn toàn việc Ngựa ghép đôi với Lừa.
      - `Horse.getBreedOffspring` & `Donkey.getBreedOffspring`:
        - Khi lai `Horse x Donkey` (hoặc `Donkey x Horse`), con sinh ra là `EntityType::MULE`.
        - Khi lai `Horse x Horse`:
          - Biến thể màu lông (Variant 0..6): 4/9 từ cha, 4/9 từ mẹ, 1/9 đột biến ngẫu nhiên trong 7 kiểu màu.
          - Vệt hoa văn (Markings 0..4): 2/5 từ cha, 2/5 từ mẹ, 1/5 đột biến ngẫu nhiên trong 5 kiểu hoa văn.
    - **Equine Attribute Inheritance Formula**:
      - `AbstractHorse.createOffspringAttribute`: Các chỉ số thể chất của con non (Max Health 15.0..30.0, Jump Strength 0.4..1.0, Movement Speed 0.1125..0.3375) được tính toán theo công thức Vanilla:
        `babyQuality = (r1 + r2 + r3) / 3.0 - 0.5`, `newValue = average + range * babyQuality`, clamped và reflected vào min/max range.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `get_equine_food_properties(item_id)`: returns healing, baby growth reduction, temper increase, and breeding capability.
    - `can_equine_mate(type_a, type_b)`: allows Horse and Donkey cross-mating while ensuring Mules remain sterile.
    - `determine_equine_baby_type(parent_a, parent_b)`: resolves Horse x Donkey hybrid to Mule, and pure breeds to Horse or Donkey.
    - `select_horse_baby_variant_and_markings`: implements 4/9 vs 4/9 vs 1/9 variant inheritance and 2/5 vs 2/5 vs 1/5 markings inheritance.
    - `calculate_offspring_attribute`: implements vanilla Gaussian quality 3-point attribute calculation.
  - Updated `HorseEntity`, `DonkeyEntity`, and `MuleEntity` in `crates/pumpkin-core/src/entity/passive/`:
    - In `mob_interact`: reads `combat::get_equine_food_properties(item.id)`, applies healing, baby growth particles (`HappyVillager`), temper increase, and golden item love mode.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - In `find_mate`: allows cross-species mating for equines via `combat::can_equine_mate`.
    - In `breed`: creates Mule for Horse x Donkey cross-breeding, applies variant & markings inheritance for Horse offspring, and calculates offspring attributes (Health, Jump, Speed) using `combat::calculate_offspring_attribute`.
## Gap 187: Llama Food Nutrition & Genetics Breeding Parity, Skeleton Horse Trap Mechanics & Water Immunity, and Zombie Horse Undead Behavior Parity
- **Vanilla Reference**:
  - `Llama.java:handleEating, getBreedOffspring, setOffspringAttributes` & `TraderLlama.java:isTraderLlama, makeNewLlama`:
    - **Llama Eating Properties & Temper Limit**:
      - Llama KHÔNG ăn các loại thực phẩm của ngựa (Đường, Táo, Cà rốt, Golden Apple...).
      - Llama chỉ ăn `Wheat` (hồi 2.0 HP, giảm thời gian lớn 10 giây/200 ticks, tăng temper +3, không kích hoạt sinh sản) và `Hay Block` (hồi 10.0 HP, giảm thời gian lớn 90 giây/1800 ticks, tăng temper +6, kích hoạt sinh sản `setInLove` nếu đã thuần hóa và là con trưởng thành).
      - Điểm thuần hóa tối đa của Llama là `30` (thay vì 100 như ngựa thông thường).
    - **Llama Genetics Breeding**:
      - `select_llama_baby_variant`: Llama con nhận 50% biến thể màu lông (`LlamaVariant`) từ cha hoặc mẹ.
      - `calculate_llama_baby_strength`: Llama con kế thừa sức chứa `strength` (1..5): lấy `max(parent_a, parent_b)`, và có 3% cơ hội đột biến tăng thêm 1 điểm (`+1`), kẹp tối đa 5 điểm.
  - `SkeletonHorse.java:mobInteract, tick, addAdditionalSaveData, readAdditionalSaveData` & `SkeletonTrapGoal.java`:
    - **Skeleton Horse Trap Mechanics**:
      - Skeleton Horse hoang dã (`!isTamed()`) có thể là một Skeleton Trap (`isTrap = true`). Ngựa hoang dã không cho phép người chơi cưỡi để thuần hóa thông thường (`mobInteract` pass/từ chối).
      - Bẫy đếm `trapTime` mỗi tick. Nếu vượt quá 18000 ticks (15 phút) mà không ai kích hoạt, bẫy sẽ tự động despawn.
      - Khi có người chơi tiếp cận trong bán kính 10 blocks (`distanceToSqr(player) < 100.0`), bẫy kích hoạt: tạo tia sét, giải phóng bẫy, biến thành ngựa đã thuần hóa và triệu hồi Skeleton kỵ sĩ.
    - **Water Breathing & Drowning Immunity**:
      - Skeleton Horse và Zombie Horse là mob undead (thuộc `#minecraft:undead` và `#minecraft:can_breathe_under_water`), không bao giờ ngạt thở hoặc chết đuối dưới nước (`can_breathe_under_water = true`, `can_drown = false`).
  - `ZombieHorse.java:mobInteract, isFood, addBehaviourGoals, canFallInLove, canAgeUp`:
    - **Zombie Horse Food & Interact**:
      - Thức ăn duy nhất của Zombie Horse là Nấm Đỏ (`ItemTags.ZOMBIE_HORSE_FOOD` / `minecraft:red_mushroom`).
      - Ăn Nấm Đỏ giúp hồi 3.0 HP và tăng temper +3.
      - Zombie Horse không thể sinh sản (`canFallInLove = false`) và con non không lớn lên bằng thức ăn (`canAgeUp = false`).
      - Khi chưa thuần hóa (`!isTame`), nếu người chơi cầm vật phẩm khác và tương tác, Zombie Horse sẽ kêu giận dữ (`Sound::EntityZombieHorseAngry`) và từ chối.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `get_llama_food_properties(item_id)`: returns nutrition properties for Wheat and Hay Block.
    - `calculate_llama_baby_strength(parent_a, parent_b, rand_int, rand_float)`: calculates offspring strength (1..5, max of parents + 3% mutation).
    - `select_llama_baby_variant(parent_a, parent_b, rand_bool)`: handles 50/50 parental coat color inheritance.
    - `should_skeleton_trap_trigger(is_trap, distance)`: checks trigger within 10 blocks.
    - `should_skeleton_trap_despawn(is_trap, trap_time)`: checks despawn after 18000 ticks.
  - Updated `LlamaEntity` in `crates/pumpkin-core/src/entity/passive/llama.rs` and `TraderLlamaEntity` in `trader_llama.rs`:
    - Implemented nutrition eating for Wheat and Hay Block, baby growth particles (`HappyVillager`), temper clamp (max 30), and Hay Block love mode.
  - Updated `SkeletonHorseEntity` in `crates/pumpkin-core/src/entity/passive/skeleton_horse.rs`:
    - Added `is_trap` and `trap_time` fields, getters/setters, and NBT serialization (`SkeletonTrap`, `SkeletonTrapTime`).
    - Implemented tick trap countdown (18000 ticks) and player proximity trigger (10 blocks).
    - Restricted untamed mounting interaction.
  - Updated `ZombieHorseEntity` in `crates/pumpkin-core/src/entity/passive/zombie_horse.rs`:
    - Switched food and tempt goal to `Item::RED_MUSHROOM`.
    - Handled Red Mushroom healing (3.0 HP) and temper increment (+3).
    - Handled angry sound rejection when untamed and interacting with items.
    - Implemented `can_breathe_under_water() -> true` and `can_drown() -> false`.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added Llama and Trader Llama variant and strength genetics inheritance.
## Gap 188: Mooshroom Breeding Variant & Rare Mutation, Pig Variant & Sound Variant Inheritance, and Sheep Wool Color Genetics & Dye Blending Parity
- **Vanilla Reference**:
  - `MushroomCow.java:getBreedOffspring, getOffspringVariant, mobInteract, addAdditionalSaveData, readAdditionalSaveData`:
    - **Mooshroom Breeding Variant Inheritance & Rare Mutation**:
      - `MushroomCow.getOffspringVariant`:
        - Nếu cả cha và mẹ cùng loại biến thể (`parent_a == parent_b`) và xúc xắc `random.nextInt(1024) == 0`: con sinh ra xảy ra đột biến biến thể hiếm sang màu đối diện (Red -> Brown, hoặc Brown -> Red).
        - Nếu không đột biến: 50% cơ hội nhận biến thể lông từ cha, 50% từ mẹ (`random.nextBoolean() ? variant : mateVariant`).
      - Trước đây Neth sinh con Mooshroom luôn mặc định ra Red (`MooshroomVariant::Red`), hoàn toàn không kế thừa từ cha mẹ hay có tỷ lệ đột biến 1/1024.
    - **Brown Mooshroom Flower Feeding**:
      - `MushroomCow.mobInteract`:
        - Khi cho Brown Mooshroom trưởng thành ăn hoa nhỏ (`MINECRAFT_SMALL_FLOWERS`), nếu con bò ĐÃ CÓ hiệu ứng canh (`stewEffects != null`), nó sẽ từ chối tiêu hao thêm hoa và chỉ phát ra hạt khói (`Particle::Smoke`).
        - Nếu chưa có hiệu ứng canh: tiêu hao 1 hoa, gán hiệu ứng, tạo hạt phép thuật (`Particle::Effect`), phát âm thanh `Sound::EntityMooshroomEat`.
        - Hiệu ứng canh được lưu vào NBT (`stew_effects` / `EffectId`).
  - `Pig.java:getBreedOffspring, addAdditionalSaveData, readAdditionalSaveData`:
    - **Pig Variant & Sound Variant Inheritance**:
      - `Pig.getBreedOffspring`: Heo con sinh ra kế thừa 50/50 biến thể ngoại hình (`PigVariant`) và 50/50 biến thể âm thanh (`PigSoundVariant`) từ cha hoặc mẹ.
      - Trước đây Neth không kế thừa variant hay sound variant cho heo con khi sinh sản trong `BreedGoal`.
  - `Sheep.java:getBreedOffspring` & `DyeColor.java:getMixedColor`:
    - **Sheep Wool Color Genetics & Dye Mixing**:
      - `Sheep.getBreedOffspring`: Cừu con sinh ra kế thừa màu lông bằng công thức pha màu vanilla:
        - Kiểm tra xem màu của cha và mẹ có thể kết hợp (crafting dye mixing) thành màu mới không:
          - White (0) + Black (15) => Gray (7)
          - White (0) + Gray (7) => Light Gray (8)
          - White (0) + Red (14) => Pink (6)
          - White (0) + Green (13) => Lime (5)
          - White (0) + Blue (11) => Light Blue (3)
          - Red (14) + Yellow (4) => Orange (1)
          - Red (14) + Blue (11) => Purple (10)
          - Green (13) + Blue (11) => Cyan (9)
          - Purple (10) + Pink (6) => Magenta (2)
        - Nếu không pha được (hoặc cùng màu): 50% cơ hội nhận màu của cha, 50% cơ hội nhận màu của mẹ.
      - Trước đây Neth sinh cừu con luôn có màu trắng mặc định (`White`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `select_mooshroom_baby_variant(parent_a, parent_b, rand_1024, rand_bool)`: handles 50/50 variant inheritance and 1/1024 rare mutation.
    - `select_pig_baby_variant_and_sound(parent_a_var, parent_b_var, parent_a_sound, parent_b_sound, rand_var, rand_sound)`: handles 50/50 pig variant and sound variant inheritance.
    - `get_mixed_sheep_color(color_a, color_b, rand_bool)`: handles vanilla 9-recipe dye color blending with 50/50 fallback.
  - Updated `PigEntity` in `crates/pumpkin-core/src/entity/passive/pig.rs`:
    - Added `get_variant(&self) -> PigVariant` and `get_sound_variant(&self) -> PigSoundVariant`.
  - Updated `MooshroomEntity` in `crates/pumpkin-core/src/entity/passive/mooshroom.rs`:
    - Handled flower feeding: prevents consuming flower if stew effect is already present (spawning smoke particles instead), and saves/loads `EffectId` in NBT.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added Mooshroom variant & 1/1024 mutation inheritance.
    - Added Pig variant & sound variant inheritance.
    - Added Sheep wool color dye mixing inheritance.
## Gap 189: Oviparous & Special Breeding Mechanics (Sniffer Egg Laying, Turtle Gravid Egg Mechanics) and Strider Eating Sound & Immersion Parity
- **Vanilla Reference**:
  - `Sniffer.java:345-352`:
    - `Sniffer.spawnChildFromBreeding(level, partner)`:
      - Sniffer sinh sản KHÔNG spawn entity mob con trực tiếp!
      - Nó đẻ ra một `ItemEntity` chứa `Items.SNIFFER_EGG` tại tọa độ của Sniffer.
      - Phát âm thanh `SoundEvents.SNIFFER_EGG_PLOP` với cao độ ngẫu nhiên `(rand - rand) * 0.2 + 0.5`.
      - Cập nhật thống kê `ANIMALS_BRED`, kích hoạt advancement criteria `bred` và `minecraft:sniffer`.
      - Rơi 1..7 điểm kinh nghiệm (`ExperienceOrb`).
      - Trước đây Neth xử lý Sniffer trong `BreedGoal` giống mob thông thường, tự động spawn entity con non ngay tại chỗ mà không gọi `spawn_child_from_breeding`.
  - `Turtle.java:310-327`:
    - `Turtle.TurtleBreedGoal`:
      - Khi hai con rùa hoàn thành giao phối, chúng KHÔNG sinh ra rùa con ngay lập tức.
      - Rùa mẹ được đánh dấu mang trứng `hasEgg = true` (`setHasEgg(true)`), rơi 1..7 kinh nghiệm, reset love ticks và cooldown 6000 ticks.
      - Rùa mẹ mang trứng sau đó thực hiện mục tiêu tìm về bãi cát quê hương (`TurtleGoHomeGoal`) để đào cát đẻ trứng (`TurtleLayEggGoal`).
      - Trước đây Neth xử lý rùa giao phối sinh ra rùa con trực tiếp thay vì đánh dấu `has_egg = true`.
  - `Strider.java:422-434`:
    - Khi Strider ăn thức ăn (`WARPED_FUNGUS`), nếu không câm (`!isSilent`), phát âm thanh `SoundEvents.STRIDER_EAT` với cao độ `1.0 + (rand - rand) * 0.2` (thay vì âm thanh `STRIDER_AMBIENT`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `get_sniffer_egg_plop_pitch(rand_a, rand_b)`: computes `(rand_a - rand_b) * 0.2 + 0.5`.
    - `get_strider_eat_sound_pitch(rand_a, rand_b)`: computes `1.0 + (rand_a - rand_b) * 0.2`.
    - `is_special_oviparous_breeding(entity_type)`: checks if entity is Sniffer or Turtle.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Delegated Sniffer breeding to `sniffer.spawn_child_from_breeding(mate)`.
    - Handled Turtle breeding: sets `turtle.set_has_egg(true)`, awards statistics/advancements, drops XP, and returns without spawning baby turtle.
  - Updated `SnifferEntity` in `crates/pumpkin-core/src/entity/passive/sniffer.rs`:
    - Applied vanilla egg plop sound pitch via `combat::get_sniffer_egg_plop_pitch`.
  - Updated `StriderEntity` in `crates/pumpkin-core/src/entity/passive/strider.rs`:
    - In `mob_interact`: plays `Sound::EntityStriderEat` with pitch from `combat::get_strider_eat_sound_pitch`.
  - Added unit test `test_oviparous_and_special_breeding_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 75/75 tests passing in `entity::combat::tests`.

## Gap 190: Rabbit Breeding Genetics & Biome Mutation, Parrot Seed Taming, Sitting & Sound Parity
- **Vanilla Reference**:
  - `Rabbit.java:342-358, 424-434`:
    - `Rabbit.getBreedOffspring(level, partner)`:
      - 95% cơ hội (`random.nextInt(20) != 0`): Thỏ con thừa hưởng 50/50 biến thể từ thỏ bố hoặc thỏ mẹ (`random.nextBoolean() ? rabbitPartner.getVariant() : this.getVariant()`).
      - 5% cơ hội (`random.nextInt(20) == 0`): Thỏ con đột biến biến thể ngẫu nhiên dựa trên quần xã sinh vật (`getRandomRabbitVariant(level, pos)`):
        - Quần xã tuyết/trắng (`SPAWNS_WHITE_RABBITS`): 80% Thỏ Trắng (id 1), 20% Thỏ Đốm Trắng (id 3).
        - Quần xã sa mạc (`SPAWNS_GOLD_RABBITS`): 100% Thỏ Vàng (id 4).
        - Quần xã thông thường: 50% Thỏ Nâu (id 0), 40% Thỏ Muối Tiêu (id 5), 10% Thỏ Đen (id 2).
        - Tuyệt đối không đột biến ra Thỏ Sát Nhân (`EVIL`, id 99).
      - Trước đây Neth hoàn toàn không xử lý biến thể cho thỏ con trong `BreedGoal`, con non sinh ra luôn nhận biến thể mặc định Nâu (0).
  - `Parrot.java:252-298, 156-160`:
    - Vẹt không thể sinh sản (`canBeABaby() -> false`, `isFood() -> false`).
    - Thuần hóa bằng các loại hạt giống (`ItemTags.PARROT_FOOD`):
      - Tiêu hao 1 item thức ăn (nếu không ở creative mode).
      - Phát âm thanh `SoundEvents.PARROT_EAT` với cao độ `1.0 + (rand - rand) * 0.2`.
      - Xác suất thuần hóa thành công là 1/10 (`random.nextInt(10) == 0`).
      - Khi thành công: trở thành thú nuôi thuần hóa (`tame(player)`), phát hạt Tim (`EntityStatus::TamingSucceeded` / event byte 7).
      - Khi thất bại: phát hạt Khói (`EntityStatus::TamingFailed` / event byte 6).
    - Khi đã thuần hóa:
      - Người chơi sở hữu click chuột phải khi vẹt ở dưới đất (`!isFlying`) sẽ chuyển đổi tư thế ngồi/đứng (`setOrderedToSit(!isOrderedToSit())`).
    - Khi ăn thức ăn độc hại (`ItemTags.PARROT_POISONOUS_FOOD`, như Cookie):
      - Nhận hiệu ứng Poison 900 ticks và chịu sát thương `Float.MAX_VALUE` chết ngay lập tức.
    - Trước đây Neth chưa triển khai `TamableAnimal` cho `ParrotEntity`, không hỗ trợ thuần hóa bằng hạt giống và chuyển đổi tư thế ngồi.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `select_rabbit_baby_variant(parent_a, parent_b, rand_20, rand_bool, rand_100, biome_type) -> i32`: selects variant matching vanilla 95% inheritance and 5% biome mutation formula.
    - `get_parrot_eat_sound_pitch(rand_a, rand_b) -> f32`: computes `1.0 + (rand_a - rand_b) * 0.2`.
    - `calculate_parrot_tame_success(rand_10: u32) -> bool`: returns `rand_10 == 0` (10% chance).
    - `RabbitBiomeSpawnType`: enum representing `White`, `Gold`, `Normal` biomes.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added Rabbit breeding genetics and mutation variant inheritance.
  - Updated `ParrotEntity` in `crates/pumpkin-core/src/entity/passive/parrot.rs`:
    - Added `ParrotVariant` enum (`RedBlue`, `Blue`, `Green`, `YellowBlue`, `Gray`).
    - Implemented `TamableAnimal` and `Animal` (`is_food -> false`) for `ParrotEntity`.
    - Added `SitWhenOrderedToGoal` and `FollowOwnerGoal` to Parrot goal selector.
    - Implemented seed taming in `mob_interact` with 1/10 success rate, eating sound, heart/smoke particles, and sit/stand toggle.
    - Added NBT serialization for Parrot variant and tamable status.
  - Added unit test `test_rabbit_and_parrot_parity` in `crates/pumpkin-core/src/entity/combat.rs` and `test_parrot_variants_and_ids` in `parrot.rs`. Total 76/76 tests passing in `entity::combat::tests`.

## Gap 191: Chicken Egg Laying Sound Pitch & Jockey Mechanics, Chicken & Cow 50/50 Breeding Variant Inheritance
- **Vanilla Reference**:
  - `Chicken.java:175-182` & `Cow.java:72-79`:
    - `Chicken.getBreedOffspring(level, partner)`:
      - `baby.setVariant(this.random.nextBoolean() ? this.getVariant() : partnerChicken.getVariant())`: gà con thừa hưởng 50/50 biến thể từ gà bố hoặc gà mẹ (`ChickenVariant`: Temperate, Warm, Cold).
    - `Cow.getBreedOffspring(level, partner)`:
      - `baby.setVariant(this.random.nextBoolean() ? this.getVariant() : partnerCow.getVariant())`: bò con thừa hưởng 50/50 biến thể từ bò bố hoặc bò mẹ (`CowVariant`: Temperate, Warm, Cold).
    - Trước đây Neth hoàn toàn không xử lý biến thể cho gà con và bò con trong `BreedGoal`, con non sinh ra luôn nhận biến thể mặc định của biome thay vì thừa hưởng từ cha mẹ.
  - `Chicken.java:75-80, 135-142, 199-201`:
    - `isChickenJockey`: boolean.
      - Nếu `isChickenJockey == true`, con gà KHÔNG BAO GIỜ đẻ trứng (`!this.isBaby() && !this.isChickenJockey() && --this.eggTime <= 0`).
      - Khi chết, gà jockey rơi 10 kinh nghiệm (`this.isChickenJockey() ? 10 : super.getBaseExperienceReward(level)`), thay vì 1..3 XP của gà thông thường.
      - Lưu/đọc NBT: `IsChickenJockey` boolean.
    - Âm thanh đẻ trứng:
      - `SoundEvents.CHICKEN_EGG`: âm lượng `1.0F`, cao độ ngẫu nhiên `(rand_a - rand_b) * 0.2F + 1.0F`.
      - Trước đây Neth phát âm thanh đẻ trứng mặc định không có pitch ngẫu nhiên và không kiểm tra jockey.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Added helper functions in `crates/pumpkin-core/src/entity/combat.rs`:
    - `select_chicken_or_cow_baby_variant(parent_a, parent_b, rand_bool) -> u8`: returns 50/50 parent variant.
    - `get_chicken_egg_lay_sound_pitch(rand_a, rand_b) -> f32`: computes `(rand_a - rand_b) * 0.2 + 1.0`.
    - `should_chicken_lay_egg(is_baby, is_jockey, egg_time) -> bool`: returns `!is_baby && !is_jockey && egg_time <= 0`.
    - `get_chicken_xp_reward(is_jockey, default_xp) -> u32`: returns 10 if jockey, else default 1..3 XP.
  - Updated `CowEntity` in `crates/pumpkin-core/src/entity/passive/cow.rs`:
    - Added `get_variant(&self) -> CowVariant`.
  - Updated `ChickenEntity` in `crates/pumpkin-core/src/entity/passive/chicken.rs`:
    - Added `is_chicken_jockey: AtomicBool`, getter/setter `is_chicken_jockey` and `get_variant`.
    - Updated NBT serialization to save/load `IsChickenJockey`.
    - Updated `mob_tick`: guarded egg laying with `should_chicken_lay_egg` and played `Sound::EntityChickenEgg` with vanilla pitch.
  - Updated `crates/pumpkin-core/src/entity/mob/mod.rs`:
    - In `get_base_experience_reward`: handled chicken jockey rewarding 10 XP.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Added 50/50 variant inheritance for Chicken and Cow.
  - Added unit test `test_chicken_and_cow_parity` in `crates/pumpkin-core/src/entity/combat.rs`. Total 77/77 tests passing in `entity::combat::tests`.

## Gap 192: Frog Oviparous Pregnancy Reproduction & Fox Trusted Entities Breeding Inheritance Parity
- **Vanilla Reference**:
  - `Frog.java:250-260`, `FrogAi.java:144-168`, `TryLaySpawnOnFluidNearLand.java:16-53`:
    - `canBeABaby() -> false`: Ếch trưởng thành không bao giờ là con non (baby stage duy nhất của loài ếch là Nòng nọc - Tadpole).
    - `spawnChildFromBreeding(level, partner)`:
      - Ếch KHÔNG sinh con non trực tiếp (`from_type` với age -24000).
      - Thay vào đó, ếch mẹ chuyển sang trạng thái mang thai / mang trứng (`getBrain().setMemory(MemoryModuleType.IS_PREGNANT, Unit.INSTANCE)`).
      - Rơi 1..7 kinh nghiệm (XP orb), kích hoạt advancement `husbandry/breed_an_animal` và `husbandry/bred_all_animals`, phát hiệu ứng hạt tim.
      - Ếch mang thai tìm vị trí cạn sát nước (`onGround && !isInWater`) và đẻ khối `Blocks.FROGSPAWN` tại bề mặt nước sát bờ, phát âm thanh `SoundEvents.FROG_LAY_SPAWN`, và xóa trạng thái `is_pregnant`.
      - NBT: `HasEgg` (boolean).
    - Trước đây Neth sinh ra ếch con thu nhỏ trực tiếp với age -24000, hoàn toàn sai lệch với cơ chế sinh sản đẻ trứng của Vanilla ếch.
  - `Fox.java:120-125, 187, 325, 417-431, 436, 447, 870-885`:
    - Tracked Data: `DATA_TRUSTED_ID_0` (index 20) và `DATA_TRUSTED_ID_1` (index 21) lưu `Optional<UUID>` của 2 đối tượng được tin tưởng.
    - Cáo con khi sinh ra thừa hưởng danh sách Trusted Entities từ cả hai người chơi cho bố mẹ ăn (`animalLoveCause` và `partnerLoveCause`).
    - Biến thể: Cáo con thừa hưởng 50/50 biến thể từ cáo bố hoặc mẹ (`Red` hoặc `Snow`).
    - Né tránh người chơi (`AvoidEntityGoal`): Cáo bỏ chạy khỏi người chơi trong bán kính 16 khối TRỪ KHI người chơi đó nằm trong danh sách được tin tưởng (`Fox.this.trusts(entity)`) hoặc cáo đang ở trạng thái phòng thủ (`isDefending()`).
    - NBT: `Trusted` (danh sách UUIDs) và `Trusted0` / `Trusted1`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `EntityType::FROG` to `is_special_oviparous_breeding`.
    - Added `should_fox_avoid_player(is_defending, is_trusted) -> bool`.
    - Added `should_frog_lay_spawn_block(is_pregnant, is_on_ground, is_in_water) -> bool`.
    - Added unit test `test_frog_and_fox_parity`.
  - Updated `FrogEntity` in `crates/pumpkin-core/src/entity/passive/frog.rs`:
    - Added `is_pregnant: AtomicBool`, getter `is_pregnant(&self) -> bool`, and setter `set_pregnant(&self, bool)`.
    - Overrode `can_be_a_baby(&self) -> bool { false }` in `impl AgeableMob for FrogEntity`.
    - Handled `HasEgg` in `mob_write_nbt` and `mob_read_nbt`.
    - Handled lay spawn block on fluid near land in `mob_tick`, playing `Sound::EntityFrogLaySpawn`.
    - Added unit test `test_frog_variants_and_oviparous`.
  - Updated `FoxEntity` in `crates/pumpkin-core/src/entity/passive/fox.rs`:
    - Added `trusted_0` and `trusted_1` fields using `crossbeam::atomic::AtomicCell<Option<Uuid>>`.
    - Added `get_trusted_0`, `get_trusted_1`, `add_trusted_uuid`, `clear_trusted`, and `trusts(&self, &Uuid) -> bool`.
    - Registered and synced `DATA_TRUSTED_ID_0` and `DATA_TRUSTED_ID_1`.
    - Implemented NBT serialization for `Trusted` int-array list and `Trusted0`/`Trusted1`.
    - Updated `AvoidEntityGoal` for `Player` with predicate checking `should_fox_avoid_player`.
    - Added unit test `test_fox_variants_and_ids`.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Handled Frog breeding: sets `frog.set_pregnant(true)`, awards stats & advancements, spawns 1..7 XP, emits heart particles, and returns without spawning baby frog.
    - Handled Fox breeding: assigns 50/50 variant and transfers both breeders' UUIDs to the baby fox.
  - All unit tests verified and passing (78/78 tests in `entity::combat::tests`, `test_fox_variants_and_ids`, and `test_frog_variants_and_oviparous`).

## Gap 193: Goat Eating Sound & Milking Parity, Ocelot Ageable Lifecycle & Trust Feeding Parity
- **Vanilla Reference**:
  - `Goat.java:148-150, 200-210, 218-233`:
    - `playEatingSound()`:
      - Khi cho dê ăn, dê phát âm thanh ăn đặc thù: `this.isScreamingGoat() ? SoundEvents.GOAT_SCREAMING_EAT : SoundEvents.GOAT_EAT` với cao độ pitch ngẫu nhiên `Mth.randomBetween(this.level().getRandom(), 0.8F, 1.2F)`.
      - Trước đây Neth phát `ambient_sound` khi cho ăn dê thay vì âm thanh ăn chuyên biệt của loài dê.
  - `Ocelot.java:63, 76-83, 98-100, 167-186, 213-222, 224-227`:
    - `DATA_TRUSTING` (boolean): Mèo gấm không thể thuần hóa (untamable) như mèo nhà, nhưng có trạng thái "trusting" (tin tưởng người chơi) khi được cho ăn cá sống (`COD`, `SALMON`) trong khi đang bị hấp dẫn bởi thức ăn (`TemptGoal`).
    - Tỉ lệ tin tưởng: `this.random.nextInt(3) == 0` (1/3 cơ hội thành công, phát hạt tim `Heart` hoặc hạt khói `Smoke` khi thất bại).
    - Né tránh người chơi (`AvoidEntityGoal<Player>`): Chỉ kích hoạt khi `!this.isTrusting()`.
    - Vòng đời tăng trưởng (`AgeableMob`): Mèo gấm là `Animal` và kế thừa vòng đời `AgeableMob` (con non, thời gian lớn `age`, `AgeLocked`, `ForcedAge` trong NBT, v.v.). Trước đây `OcelotEntity` trong Neth chưa implement `AgeableMob`, dẫn đến thiếu method `as_ageable`, không thể tăng trưởng qua thời gian (`ageable_ai_step`), và thiếu NBT ageable.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `get_goat_eat_sound_pitch(rand: f32) -> f32`: computes `0.8 + rand * 0.4`.
    - Added `calculate_ocelot_trust_success(rand_3: u32) -> bool`: returns `rand_3 == 0` (1 in 3).
    - Added `should_ocelot_avoid_player(is_trusting: bool) -> bool`: returns `!is_trusting`.
    - Added unit test `test_goat_eating_and_ocelot_trust_parity`.
  - Updated `GoatEntity` in `crates/pumpkin-core/src/entity/passive/goat.rs`:
    - Updated `mob_interact` to use `Sound::EntityGoatScreamingEat` / `Sound::EntityGoatEat` instead of ambient sound when fed.
  - Updated `OcelotEntity` in `crates/pumpkin-core/src/entity/passive/ocelot.rs`:
    - Added `ageable_data: AgeableData` to `OcelotEntity`.
    - Implemented `AgeableMob for OcelotEntity` and added `as_ageable(&self) -> Option<&dyn AgeableMob>` in `impl Mob`.
    - Updated `mob_tick` to call `self.ageable_ai_step()`.
    - Updated `mob_write_nbt` and `mob_read_nbt` to serialize/deserialize ageable data.
    - Updated `mob_interact` to use `combat::calculate_ocelot_trust_success`.
    - Added unit test `test_ocelot_trust_and_ageable`.
  - All unit tests verified and passing (79/79 tests in `entity::combat::tests`, `test_ocelot_trust_and_ageable`).

## Gap 194: Bat Voice Pitch & Roosting Wake Conditions, Polar Bear Neutral Anger NBT & Standing Parity
- **Vanilla Reference**:
  - `Bat.java:66-68, 135-151, 198-208`:
    - `getVoicePitch()`: Vanilla computes `super.getVoicePitch() * 0.95F` where living entity base voice pitch is `(rand_a - rand_b) * 0.2F + 1.0F`, giving range `[0.76, 1.14]` centered at `0.95`. Previously Neth hardcoded a static `0.95`.
    - Roosting Wake Conditions (`tick_roosting`): A resting bat wakes up and takes flight if:
      - The block above is no longer solid (`!above_state.is_solid_block()`), or
      - A player is within 4.0 blocks distance (`player_distance < 4.0`).
      - Upon taking damage (`on_damage`), the bat immediately wakes up and flies away.
      - Takeoff sound (`Sound::EntityBatTakeoff`) and ambient sound use the dynamic voice pitch.
  - `PolarBear.java:120-129, 176-181, 205-207`, `NeutralMob.java:34-56`:
    - Polar Bear extends `NeutralMob` and `AgeableMob`.
    - In Neth, `NeutralMob::read_anger_nbt` now supports legacy `AngryAt` fallback alongside modern `angry_at` UUID tag.
    - Cleaned up `PolarBearEntity::mob_write_nbt` and `mob_read_nbt` to avoid duplicate ageable/neutral serialization (managed automatically by `Mob::write_custom_nbt` / `read_custom_nbt`).
    - Standing on hind legs and growling warning sound conditions verified (`dist_sq > (width + 3.0)^2 && dist_sq <= (width + 5.0)^2`).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `get_bat_voice_pitch(rand_a: f32, rand_b: f32) -> f32`.
    - Added `should_bat_wake_from_resting(is_solid_above: bool, player_distance: Option<f64>) -> bool`.
    - Added unit test `test_bat_and_polar_bear_parity`.
  - Updated `BatEntity` in `crates/pumpkin-core/src/entity/mob/bat.rs`:
    - `tick_ambient_sound` and `tick_roosting` / `on_damage` takeoff sounds now use `combat::get_bat_voice_pitch`.
    - `tick_roosting` uses `combat::should_bat_wake_from_resting`.
    - Added unit test `test_bat_voice_pitch_and_roosting_wake`.
  - Updated `NeutralMob` in `crates/pumpkin-core/src/entity/mob/neutral.rs`:
    - Supported both `angry_at` and `AngryAt` UUID in `read_anger_nbt`.
  - Updated `PolarBearEntity` in `crates/pumpkin-core/src/entity/passive/polar_bear.rs`:
    - Cleaned up redundant NBT calls.
    - Added unit test `test_polar_bear_warning_and_standing_conditions`.
  - All unit tests verified and passing (80/80 tests in `entity::combat::tests`, `test_bat_voice_pitch_and_roosting_wake`, and `test_polar_bear_warning_and_standing_conditions`).

## Gap 195: Llama Natural Strength Distribution & Chest Sounds, Trader Llama DespawnDelay & Sterile Parity
- **Vanilla Reference**:
  - `Llama.java:93-96, 225-230, 286, 296, 300-302, 315-317`:
    - Natural Spawn Strength: Khi sinh ra tự nhiên, `maxStrength = random.nextFloat() < 0.04F ? 5 : 3; strength = 1 + random.nextInt(maxStrength);` (96% cơ hội sức mạnh 1..=3, 4% hiếm có sức mạnh 1..=5). Trước đây Neth random đều từ 1..=5.
    - Chest Equip & Eating Sounds:
      - Khi gắn rương lên Llama, phát `SoundEvents.LLAMA_CHEST` (`Sound::EntityLlamaChest`) với pitch `1.0 + (rand_a - rand_b) * 0.2` (trước đây Neth phát `Sound::EntityDonkeyChest`).
      - Khi cho Llama ăn thức ăn, phát `SoundEvents.LLAMA_EAT` (`Sound::EntityLlamaEat`) với pitch `1.0 + (rand_a - rand_b) * 0.2` (trước đây Neth phát `Sound::EntityHorseEat`).
    - Inventory Columns: Số cột hòm đồ của Llama: `hasChest() ? strength : 0` (1..=5 cột, mỗi cột 3 ô).
  - `TraderLlama.java:28-29, 37-39, 53, 59, 71-73`:
    - DespawnDelay: Mặc định `47999` ticks, được serialize và deserialize trong NBT qua khóa `DespawnDelay`. Trước đây Neth thiếu trường này trong `TraderLlamaEntity`.
    - Sterile (Không thể phối giống): `TraderLlama` trong vanilla không thể phối giống (`canMate` trả về `false`). Không có `BreedGoal` trong goal selector, và khi ăn HayBlock chỉ hồi máu / tăng temper chứ không bao giờ rơi vào trạng thái `InLove`. Trong `BreedGoal::find_mate`, Trader Llama bị loại bỏ khỏi danh sách bạn tình hợp lệ.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `calculate_initial_llama_strength(rand_float: f32, rand_int: u32) -> i32`.
    - Added `get_llama_sound_pitch(rand_a: f32, rand_b: f32) -> f32`.
    - Added `get_llama_inventory_columns(has_chest: bool, strength: i32) -> i32`.
    - Added `can_llama_mate(partner_is_llama: bool, is_trader_llama: bool) -> bool`.
    - Added unit test `test_llama_and_trader_llama_parity`.
  - Updated `LlamaEntity` in `crates/pumpkin-core/src/entity/passive/llama.rs`:
    - Used `combat::calculate_initial_llama_strength` in `new`.
    - Added `get_inventory_columns(&self) -> i32`.
    - Updated `mob_interact` to use `Sound::EntityLlamaChest` and `Sound::EntityLlamaEat` with randomized pitch.
    - Added unit test `test_llama_inventory_and_sound_parity`.
  - Updated `TraderLlamaEntity` in `crates/pumpkin-core/src/entity/passive/trader_llama.rs`:
    - Added `despawn_delay: AtomicI32` with default `47999`.
    - Used `combat::calculate_initial_llama_strength` in `new`.
    - Removed `BreedGoal` from `goals_selector`.
    - Added `get_despawn_delay`, `set_despawn_delay`, and `get_inventory_columns`.
    - Handled `DespawnDelay` in `mob_write_nbt` and `mob_read_nbt`.
    - Updated `mob_interact` to use `Sound::EntityLlamaChest` and `Sound::EntityLlamaEat`, removed in-love breeding trigger.
    - Added unit test `test_trader_llama_despawn_and_sterile_parity`.
  - Updated `BreedGoal` in `crates/pumpkin-core/src/entity/ai/goal/breed.rs`:
    - Excluded `EntityType::TRADER_LLAMA` from candidate selection.
  - All unit tests verified and passing (81/81 tests in `entity::combat::tests`, `test_llama_inventory_and_sound_parity`, and `test_trader_llama_despawn_and_sterile_parity`).

---

## Gap 196: Glow Squid Dark Ticks & NBT, Squid Squirt Trigger & Volume, Strider Ridden Speed & Mood Sounds

- **Vanilla Reference**:
  - `GlowSquid.java:27, 42, 71-100`:
    - `DATA_DARK_TICKS_REMAINING` (`SynchedEntityData.defineId(GlowSquid.class, EntityDataSerializers.INT)`): Data tracker id 18 cho `DarkTicksRemaining`.
    - NBT: `DarkTicksRemaining` (int). Lưu và đọc số ticks mực phát sáng còn tối lại.
    - `hurtServer`: khi bị thương, luôn set `dark_ticks = 100` (`this.setDarkTicks(100)`).
    - `aiStep`: mỗi tick nếu `dark_ticks > 0` thì giảm 1 (`this.setDarkTicks(darkTicks - 1)`). Chỉ phát hạt phát quang (`ParticleTypes.GLOW`) khi không trong dark ticks.
  - `Squid.java:93-95, 180`:
    - `hurtServer`: mực chỉ xịt mực khi bị tấn công bởi một thực thể khác (`this.getLastHurtByMob() != null`). Không tự động xịt mực khi chết cháy, ngạt thở hay rơi vào void.
    - `getSoundVolume`: âm thanh xịt mực `SoundEvents.SQUID_SQUIRT` và `SoundEvents.GLOW_SQUID_SQUIRT` có volume `0.4F` (thay vì 1.0F).
  - `Strider.java:263, 273-275, 303-307, 354-356`:
    - Tốc độ cưỡi (`getRiddenSpeed`): `(float)(base_speed * (is_suffocating ? 0.35F : 0.55F) * boost_factor)`.
    - Steering boost tick: `this.steering.tickBoost()` được tick mỗi tick để đếm ngược boost time.
    - Mood sounds: Trong `tick()`, nếu đang bị dụ bằng nấm biến dị (`isBeingTempted()`) thì có 1/140 cơ hội mỗi tick phát `SoundEvents.STRIDER_HAPPY`; nếu đang hoảng loạn tháo chạy (`isPanicking()`) thì có 1/60 cơ hội mỗi tick phát `SoundEvents.STRIDER_RETREAT`.
    - Ambient sound: chỉ phát `STRIDER_AMBIENT` khi không hoảng loạn và không bị dụ (`!this.isPanicking() && !this.isBeingTempted()`).
    - Ăn nấm: loại bỏ việc phát âm thanh `EntityStriderEat` trùng lặp 2 lần khi tương tác cho ăn.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `calculate_strider_ridden_speed(base_speed: f64, is_suffocating: bool, boost_factor: f32) -> f32`.
    - Added `should_strider_be_happy(is_tempted: bool, rand_val: u32) -> bool`.
    - Added `should_strider_retreat(is_panicking: bool, rand_val: u32) -> bool`.
    - Added `should_squid_squirt_ink(has_attacker: bool) -> bool`.
    - Added unit test `test_glow_squid_and_strider_parity`.
  - Updated `ItemSteerable` in `crates/pumpkin-core/src/entity/item_steerable.rs`:
    - Added `get_steerable_speed(&self) -> f32`.
  - Updated `GlowSquidEntity` in `crates/pumpkin-core/src/entity/passive/glow_squid.rs`:
    - Added `get_dark_ticks_remaining(&self) -> i32` and `set_dark_ticks(&self, ticks: i32)`.
    - Synced `pumpkin_data::tracked_data::glow_squid::DATA_DARK_TICKS_REMAINING` with `VarInt`.
    - Implemented `mob_init_data_tracker`, `mob_write_nbt`, and `mob_read_nbt` for `DarkTicksRemaining`.
    - In `on_damage`, always set dark ticks to 100, and only squirt ink when attacked with sound volume 0.4.
    - In `mob_tick`, decrement dark ticks when > 0 and only emit glow particles when dark ticks == 0.
    - Added unit test `test_glow_squid_dark_ticks_and_nbt`.
  - Updated `SquidEntity` in `crates/pumpkin-core/src/entity/passive/squid.rs`:
    - In `on_damage`, only squirt ink when attacked with sound volume 0.4.
    - Added unit test `test_squid_ink_trigger`.
  - Updated `StriderEntity` in `crates/pumpkin-core/src/entity/passive/strider.rs`:
    - Added `get_ridden_speed(&self) -> f32`, `is_being_tempted(&self) -> bool`, and `is_panicking(&self) -> bool`.
    - In `mob_tick`, tick `self.steering.tick_boost()`, play `Sound::EntityStriderHappy` (1/140) when tempted, `Sound::EntityStriderRetreat` (1/60) when panicking, and only play ambient sound when neither panicking nor tempted.
    - Implemented `get_steerable_speed` in `ItemSteerable`.
    - Removed duplicate `EntityStriderEat` sound playback in `mob_interact`.
    - Added unit test `test_strider_ridden_speed_and_sounds`.
  - All unit tests verified and passing (82/82 tests in `entity::combat::tests`, `test_glow_squid_dark_ticks_and_nbt`, `test_squid_ink_trigger`, and `test_strider_ridden_speed_and_sounds`).

---

## Gap 197: Dolphin Data Tracker & Moistness, Pufferfish Scale & Poison Sting Parity

- **Vanilla Reference**:
  - `Dolphin.java:75-79, 120-141, 311-324`:
    - `DATA_GOT_FISH`: id 18 (`pumpkin_data::tracked_data::dolphin::GOT_FISH`), boolean.
    - `DATA_MOISTNESS_LEVEL`: id 19 (`pumpkin_data::tracked_data::dolphin::MOISTNESS_LEVEL`), int.
    - Hằng số vanilla: `TOTAL_AIR_SUPPLY = 4800;` (4 phút nín thở), `TOTAL_MOISTNESS_LEVEL = 2400;` (2 phút giữ ẩm trên cạn).
    - `mobInteract`: chấp nhận toàn bộ cá thuộc tag `ItemTags.FISHES` (`tag::Item::MINECRAFT_FISHES`, bao gồm Cod, Salmon, Tropical Fish, và Pufferfish).
    - NBT: `GotFish` (boolean) và `Moistness` (int).
  - `Pufferfish.java:41-43, 52-55, 83, 142-145, 155-160, 181-191`:
    - `PUFF_STATE`: id data tracker cho trạng thái phồng (0: Small, 1: Mid, 2: Full).
    - Kích thước (`refreshDimensions`): tỷ lệ scale theo state: 0 -> 0.5F, 1 -> 0.7F, 2 -> 1.0F.
    - Sát thương chích (`touch` / `playerTouch`): `1 + puff_state` damage (1.0 cho Small, 2.0 cho Mid, 3.0 cho Full).
    - Hiệu ứng độc (`MobEffects.POISON`): thời lượng `60 * puff_state` ticks (0s cho Small, 3s cho Mid, 6s cho Full) ở amplifier 0.
    - NBT: `PuffState` được clamped `Math.min(val, 2).max(0)`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `get_dolphin_total_air_supply() -> i32` (4800).
    - Added `get_dolphin_total_moistness() -> i32` (2400).
    - Added `calculate_pufferfish_poison_duration(puff_state: i32) -> i32`.
    - Added `calculate_pufferfish_sting_damage(puff_state: i32) -> f32`.
    - Added `calculate_pufferfish_scale(puff_state: i32) -> f32`.
    - Added unit test `test_dolphin_and_pufferfish_parity`.
  - Updated `DolphinEntity` in `crates/pumpkin-core/src/entity/passive/dolphin.rs`:
    - Synced `GOT_FISH` and `MOISTNESS_LEVEL` with data tracker via `set_got_fish` and `set_moistness_level`.
    - Implemented `mob_init_data_tracker`, `mob_write_nbt`, and `mob_read_nbt` for `GotFish` and `Moistness`.
    - Handled `tag::Item::MINECRAFT_FISHES` and `Item::PUFFERFISH` in `mob_interact`.
    - Added unit test `test_dolphin_data_tracker_and_moistness`.
  - Updated `PufferfishEntity` in `crates/pumpkin-core/src/entity/passive/pufferfish.rs`:
    - Implemented `mob_init_data_tracker` synchronizing `tracked_data::pufferfish::PUFF_STATE`.
    - Integrated `combat::calculate_pufferfish_scale`, `calculate_pufferfish_sting_damage`, and `calculate_pufferfish_poison_duration`.
    - Clamped NBT `PuffState` in `mob_read_nbt`.
    - Added unit test `test_pufferfish_scales_and_sting`.
  - All unit tests verified and passing (83/83 tests in `entity::combat::tests`, `test_dolphin_data_tracker_and_moistness`, and `test_pufferfish_scales_and_sting`).

---

## Gap 198: Wolf Armor Repair Validation & Durability Reduction, Tail Angle, Fox Food Delay, Cat Healing Parity

- **Vanilla Reference**:
  - `Wolf.java:467-478, 532-540`:
    - `can_repair_wolf_armor`: chỉ khi chó đang ngồi (`isInSittingPose()`), đang mặc giáp sói (`isWearingBodyArmor()`), và giáp đã bị hư hại (`damage > 0`).
    - Sửa giáp: mỗi Armadillo Scute hồi phục `(int)(armor.getMaxDamage() * 0.125F)` durability (1/8 độ bền tối đa) và trừ trực tiếp điểm hư hại của giáp.
    - Đuôi chó (`getTailAngle`):
      - Khi tức giận (`isAngry`): `1.5393804F` (~88.2°).
      - Khi thuần hóa (`isTame`): `(0.55F - damageRatio * 0.4F) * PI` (với `damageRatio = (maxHealth - health) / maxHealth`).
      - Khi hoang dã: `PI / 5.0` (~36°).
  - `Fox.java:132, 213-220`:
    - `MIN_TICKS_BEFORE_EAT = 600` (30 giây). Khi cáo ngậm thức ăn trong mồm (`EquipmentSlot.MAINHAND`), nó đếm số tick ngậm. Sau 600 ticks, cáo tiêu thụ 1 item thức ăn, hồi phục 2.0 HP, phát âm thanh `SoundEvents.FOX_EAT` và reset bộ đếm về 0.
    - NBT: `TicksSinceEaten` (int).
  - `Cat.java:423-426`:
    - Khi mèo đã thuần hóa bị thương và được chủ cho ăn cá (`ItemTags.CAT_FOOD`), mèo hồi phục `1.0F` HP (nửa tim), thay vì 2.0F.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `calculate_wolf_tail_angle(is_angry: bool, is_tame: bool, health: f32, max_health: f32) -> f32`.
    - Added `calculate_wolf_armor_repair_amount(max_damage: i32) -> i32`.
    - Added `can_repair_wolf_armor(is_sitting: bool, is_damaged: bool, has_armor: bool) -> bool`.
    - Added `get_fox_min_ticks_before_eat() -> i32` (600).
    - Added `can_fox_eat_item(is_food: bool, ticks_since_eaten: i32) -> bool`.
    - Added `get_cat_food_healing() -> f32` (1.0).
    - Added unit test `test_wolf_fox_and_cat_parity`.
  - Updated `WolfEntity` in `crates/pumpkin-core/src/entity/passive/wolf.rs`:
    - Added `get_tail_angle(&self) -> f32`.
    - Fixed wolf armor repair: verified sitting pose, armor damage, reduced armor damage value by 12.5% max durability, synced equipment changes, and triggered advancement.
    - Added unit test `test_wolf_tail_and_armor_repair`.
  - Updated `FoxEntity` in `crates/pumpkin-core/src/entity/passive/fox.rs`:
    - Added `ticks_since_eaten: AtomicI32` with getters/setters and NBT persistence (`TicksSinceEaten`).
    - In `mob_tick`, tracked ticks holding food in mainhand, consumed food after 600 ticks with 2.0 HP heal and `EntityFoxEat` sound.
    - Added unit test `test_fox_ticks_since_eaten`.
  - Updated `CatEntity` in `crates/pumpkin-core/src/entity/passive/cat.rs`:
    - Adjusted feeding healing to `combat::get_cat_food_healing()` (1.0 HP).
    - Added unit test `test_cat_food_healing`.
  - All unit tests verified and passing (84/84 tests in `entity::combat::tests`, `test_wolf_tail_and_armor_repair`, `test_fox_ticks_since_eaten`, and `test_cat_food_healing`).

---

## Gap 199: Parrot Fall Damage Immunity, Rabbit Evil Sound Category & Attack Sound, Axolotl Play Dead Silence & Rehydrate, Sniffer Eating Pitch, Armadillo Scared Damage Parity

- **Vanilla Reference**:
  - `Parrot.java:293-294`:
    - `checkFallDamage` rỗng (`// No-op`), Parrots miễn nhiễm hoàn toàn với sát thương rơi (0 fall damage points).
  - `Rabbit.java:82-95, 317-326`:
    - Tốc độ chuẩn: `STROLL_SPEED_MOD = 0.6`, `BREED_SPEED_MOD = 0.8`, `FOLLOW_SPEED_MOD = 1.0`, `FLEE_SPEED_MOD = 2.2`, `ATTACK_SPEED_MOD = 1.4`.
    - `ARMOR` cho Killer Bunny: 8.0. `ATTACK_DAMAGE`: 3.0 base + 5.0 modifier (`EVIL_ATTACK_POWER_MODIFIER`) = 8.0.
    - `getSoundSource`: trả về `SoundSource.HOSTILE` nếu là Evil Rabbit (`Variant::EVIL`), ngược lại `SoundSource.NEUTRAL`.
    - `playAttackSound`: Killer Bunny phát âm thanh `SoundEvents.RABBIT_ATTACK` với pitch `(rand - rand) * 0.2 + 1.0` khi tấn công mục tiêu.
  - `Axolotl.java:169-173, 273-276`:
    - `playAmbientSound`: không phát âm thanh ambient khi đang giả chết (`if (!this.isPlayingDead()) super.playAmbientSound()`).
    - `rehydrate()`: cộng 1800 ticks air supply (`REHYDRATE_AIR_SUPPLY = 1800`), giới hạn ở `max_air_supply` (6000).
  - `Sniffer.java:360`:
    - `playEatingSound`: phát âm thanh `SoundEvents.SNIFFER_EAT` với pitch ngẫu nhiên `Mth.randomBetween(random, 0.8F, 1.2F)`.
  - `Armadillo.java:279-283`:
    - `hurtServer`: khi cuộn tròn (`isScared`), sát thương nhận vào bị giảm theo công thức: `(damage - 1.0F) / 2.0F`.
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added `calculate_parrot_fall_damage() -> i32` (0) and `should_parrot_take_fall_damage() -> bool` (false).
    - Added Rabbit speed constants: `RABBIT_STROLL_SPEED_MOD`, `RABBIT_BREED_SPEED_MOD`, `RABBIT_FOLLOW_SPEED_MOD`, `RABBIT_FLEE_SPEED_MOD`, `RABBIT_ATTACK_SPEED_MOD`, `RABBIT_EVIL_ARMOR`, `RABBIT_EVIL_ATTACK_POWER_MODIFIER`, `RABBIT_DEFAULT_ATTACK_POWER`.
    - Added `calculate_rabbit_attack_pitch(rand_a: f32, rand_b: f32) -> f32` and `is_rabbit_evil(variant_id: i32) -> bool`.
    - Added `AXOLOTL_REHYDRATE_AIR_SUPPLY = 1800` and `AXOLOTL_TOTAL_AIR_SUPPLY = 6000`.
    - Added `calculate_axolotl_rehydrate_air(current_air: i32, max_air: i32) -> i32` and `should_axolotl_play_ambient_sound(is_playing_dead: bool, is_silent: bool) -> bool`.
    - Added `calculate_sniffer_eat_pitch(rand_val: f32) -> f32` (0.8..=1.2).
    - Added `calculate_armadillo_scared_damage(raw_damage: f32) -> f32`.
    - Added unit test `test_passive_mob_details_parity`.
  - Updated `ParrotEntity` in `crates/pumpkin-core/src/entity/passive/parrot.rs`:
    - Implemented `calculate_fall_damage` returning 0 and `adjust_damage_amount` canceling `DamageType::FALL`.
    - Added unit test `test_parrot_fall_damage_immunity`.
  - Updated `RabbitEntity` in `crates/pumpkin-core/src/entity/passive/rabbit.rs`:
    - Added `get_sound_category(&self) -> SoundCategory` using `SoundCategory::Hostile` for Evil rabbit and `SoundCategory::Neutral` for normal rabbits.
    - Implemented `on_attack` playing `Sound::EntityRabbitAttack` for Killer Bunny with vanilla attack pitch.
    - Added unit test `test_rabbit_variants_and_evil_sound_category`.
  - Updated `AxolotlEntity` in `crates/pumpkin-core/src/entity/passive/axolotl.rs`:
    - Guarded ambient sound playback with `should_axolotl_play_ambient_sound` to silence playing dead axolotls.
    - Added `rehydrate(&self)` restoring 1800 air supply up to `AXOLOTL_TOTAL_AIR_SUPPLY`.
    - Added unit test `test_axolotl_rehydrate_and_play_dead_silence`.
  - Updated `SnifferEntity` in `crates/pumpkin-core/src/entity/passive/sniffer.rs`:
    - Randomized `Sound::EntitySnifferEat` pitch between 0.8 and 1.2 on feeding interaction.
    - Added unit test `test_sniffer_eating_sound_pitch_and_constants`.
  - Updated `ArmadilloEntity` in `crates/pumpkin-core/src/entity/passive/armadillo.rs`:
    - Standardized shell-reduced damage using `calculate_armadillo_scared_damage`.
    - Added unit test `test_armadillo_scared_damage_reduction`.
  - All unit tests verified and passing (85/85 tests in `entity::combat::tests`, and all passive mob unit tests).

## Gap 200: Hostile Mob Details Parity (Enderman Daylight De-aggression & Stare Sound, Blaze Crackle Burn Sound & Small Fireball Spread, Slime/MagmaCube Jump Boosts & Sound Parity, Silverfish Wake-up Duration & Endermite Lifetime Parity)
- **Vanilla Reference**:
  - `Enderman.java:77-78, 84-85, 176-183, 247-254`:
    - `customServerAiStep`: Trong ánh sáng ban ngày (`level.isBrightOutside()`), nếu Enderman đã giữ mục tiêu tối thiểu 600 ticks (`MIN_DEAGGRESSION_TIME = 600`, tương đương 30 giây kể từ `targetChangeTime`), có thể nhìn thấy bầu trời (`level.canSeeSky()`) và giá trị ánh sáng ma thuật `br > 0.5F`, nó có xác suất `random.nextFloat() * 30.0F < (br - 0.4F) * 2.0F` để xóa target (`setTarget(null)`) và dịch chuyển ngẫu nhiên đi nơi khác.
    - `playStareSound`: Khi bị người chơi nhìn chằm chằm hoặc cập nhật trạng thái `DATA_CREEPY`, nếu chưa câm (`!isSilent()`) và đã qua ít nhất 400 ticks (`DELAY_BETWEEN_CREEPY_STARE_SOUND = 400`, 20 giây) kể từ lần phát âm thanh trước, phát `SoundEvents.ENDERMAN_STARE` với âm lượng lớn `2.5F` và pitch `1.0F`.
  - `Blaze.java:90-103, 231-237`:
    - `aiStep`: Blaze phát âm thanh lửa cháy lách tách `SoundEvents.BLAZE_BURN` với tỷ lệ 1/24 mỗi tick (`random.nextInt(24) == 0`) khi không câm, âm lượng `1.0F + random.nextFloat()` và pitch `random.nextFloat() * 0.7F + 0.3F`.
    - `BlazeAttackGoal`: Khi bắn cầu lửa nhỏ `SmallFireball`, độ lệch hướng (triangular spread) tỉ lệ thuận với căn bậc 4 của khoảng cách: `spread = 2.297 * Math.sqrt(Math.sqrt(distance)) * 0.5`.
  - `MagmaCube.java:94-99, 102-109` & `AbstractCubeMob.java:288-290`:
    - `jumpFromGround`: MagmaCube nhận thêm lực đẩy nhảy tỉ lệ thuận với kích thước: `sizeJumpBoostPower = this.getSize() * 0.1F`.
    - `jumpInLiquid(LAVA)`: Khi ở trong dung nham, vận tốc nhảy đứng của MagmaCube được đặt thành `0.22F + this.getSize() * 0.05F`.
    - `doPlayJumpSound`: Khối CubeMob chỉ phát âm thanh nhảy khi `getSize() > 0`.
  - `Silverfish.java:189` & `Endermite.java:79-84`:
    - `Silverfish`: Khi bị dính sát thương, đặt `wakeUpFriendsTicks = 20` để tiếp tục đánh thức và phá vỡ các khối đá bị nhiễm (Infested Stone) xung quanh trong 20 ticks.
    - `Endermite`: Trừ khi có yêu cầu tồn tại vĩnh viễn (`isPersistenceRequired()`, ví dụ được đặt tên), Endermite tự động biến mất (despawn) sau 2400 ticks (2 phút).
- **Neth Status**: **RESOLVED & VERIFIED**
  - Updated `crates/pumpkin-core/src/entity/combat.rs`:
    - Added constants `ENDERMAN_MIN_DEAGGRESSION_TIME = 600`, `ENDERMAN_DELAY_BETWEEN_CREEPY_STARE_SOUND = 400`, `ENDERMAN_STARE_SOUND_VOLUME = 2.5`.
    - Added `can_enderman_deaggro_in_sunlight(current_tick, target_change_time) -> bool`.
    - Added `should_enderman_daylight_teleport(br: f32, rand_val: f32) -> bool`.
    - Added `should_enderman_play_stare_sound(current_tick, last_stare_sound, is_silent) -> bool`.
    - Added `calculate_blaze_burn_pitch(rand_val: f32) -> f32` (`rand * 0.7 + 0.3`) and `calculate_blaze_burn_volume(rand_val: f32) -> f32` (`1.0 + rand`).
    - Added `should_blaze_play_burn_sound(rand_int_24: u32, is_silent: bool) -> bool`.
    - Added `calculate_blaze_small_fireball_spread(distance_sq: f64) -> f64`.
    - Added `calculate_magma_cube_jump_boost(size: i32) -> f64` and `calculate_magma_cube_lava_jump_boost(size: i32) -> f64`.
    - Added `should_cube_mob_play_jump_sound(size: i32) -> bool`.
    - Added `SILVERFISH_WAKE_UP_FRIENDS_TICKS = 20`.
    - Added `ENDERMITE_LIFETIME_MAX = 2400` and `should_endermite_despawn(lifetime, persistence_required) -> bool`.
    - Added unit test `test_hostile_mob_details_parity` (total 86/86 combat tests passing).
  - Updated `EndermanEntity` in `crates/pumpkin-core/src/entity/mob/enderman.rs`:
    - Added `target_change_time: AtomicCell<u64>` and `last_stare_sound: AtomicCell<u64>`.
    - Updated `set_target`: tracks target acquisition timestamp and resets on target loss.
    - Added `play_stare_sound(&self, tick_count: u64)` playing `Sound::EntityEndermanStare` at volume 2.5 after 400 ticks cooldown.
    - Updated `mob_tick`: replaced static 1/60 roll with vanilla daylight formula checking 600 ticks de-aggression threshold and magic light level probability.
    - Added unit test `test_enderman_sunlight_and_stare_parity`.
  - Updated `BlazeEntity` in `crates/pumpkin-core/src/entity/mob/blaze.rs`:
    - Added crackling burn sound playback (`Sound::EntityBlazeBurn`) at 1/24 tick rate with randomized volume (1.0..=2.0) and pitch (0.3..=1.0).
    - Added unit test `test_blaze_burn_sound_and_fireball_spread_parity`.
  - Updated `SilverfishEntity` in `crates/pumpkin-core/src/entity/mob/silverfish.rs`:
    - Standardized friend wake-up ticks to `SILVERFISH_WAKE_UP_FRIENDS_TICKS` (20 ticks).
    - Added unit test `test_silverfish_wake_up_and_xp_parity`.
  - Updated `EndermiteEntity` in `crates/pumpkin-core/src/entity/mob/endermite.rs`:
    - Enforced despawn condition via `should_endermite_despawn` at 2400 ticks.
    - Added unit test `test_endermite_despawn_and_xp_parity`.
  - All unit tests verified and passing (86/86 in `entity::combat::tests`, and 29/29 in `entity::mob`).






















