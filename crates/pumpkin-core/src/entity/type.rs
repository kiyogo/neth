use std::sync::Arc;

use pumpkin_data::entity::{EntityType, MobCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use uuid::Uuid;

use crate::entity::boss::ender_dragon::EnderDragonEntity;
use crate::entity::boss::wither::WitherEntity;
use crate::entity::decoration::{
    armor_stand::ArmorStandEntity,
    cushion::CushionEntity,
    display::{BlockDisplayEntity, ItemDisplayEntity, TextDisplayEntity},
    end_crystal::EndCrystalEntity,
    item_frame::ItemFrameEntity,
    mannequin::MannequinEntity,
    painting::PaintingEntity,
};
use crate::entity::ominous_item_spawner::OminousItemSpawnerEntity;
use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::falling::FallingEntity;
use crate::entity::interaction::InteractionEntity;
use crate::entity::item::ItemEntity;
use crate::entity::lightning::LightningBoltEntity;
use crate::entity::living::LivingEntity;
use crate::entity::marker::MarkerEntity;
use crate::entity::mob::bat::{self, BatEntity};
use crate::entity::mob::blaze::BlazeEntity;
use crate::entity::mob::breeze::BreezeEntity;
use crate::entity::mob::cave_spider::CaveSpiderEntity;
use crate::entity::mob::creaking::CreakingEntity;
use crate::entity::mob::creeper::CreeperEntity;
use crate::entity::mob::cube_mob::CubeMobEntity;
use crate::entity::mob::elder_guardian::ElderGuardianEntity;
use crate::entity::mob::enderman::EndermanEntity;
use crate::entity::mob::endermite::EndermiteEntity;
use crate::entity::mob::evoker::EvokerEntity;
use crate::entity::mob::ghast::GhastEntity;
use crate::entity::mob::giant::GiantEntity;
use crate::entity::mob::guardian::GuardianEntity;
use crate::entity::mob::hoglin::HoglinEntity;
use crate::entity::mob::illusioner::IllusionerEntity;
use crate::entity::mob::magma_cube::MAGMA_CUBE;
use crate::entity::mob::phantom::PhantomEntity;
use crate::entity::mob::piglin::PiglinEntity;
use crate::entity::mob::piglin_brute::PiglinBruteEntity;
use crate::entity::mob::pillager::PillagerEntity;
use crate::entity::mob::ravager::RavagerEntity;
use crate::entity::mob::shulker::ShulkerEntity;
use crate::entity::mob::silverfish::SilverfishEntity;
use crate::entity::mob::skeleton::{
    bogged::BoggedSkeletonEntity, parched::ParchedSkeletonEntity, skeleton::SkeletonEntity,
    stray::StraySkeletonEntity, wither::WitherSkeletonEntity,
};
use crate::entity::mob::slime::{SLIME, check_slime_spawn_rules};
use crate::entity::mob::spider::SpiderEntity;
use crate::entity::mob::sulfur_cube::SULFUR_CUBE;
use crate::entity::mob::vex::VexEntity;
use crate::entity::mob::vindicator::VindicatorEntity;
use crate::entity::mob::warden::WardenEntity;
use crate::entity::mob::witch::WitchEntity;
use crate::entity::mob::zoglin::ZoglinEntity;
use crate::entity::mob::zombie::zombie_villager::ZombieVillagerEntity;
use crate::entity::mob::zombie::{drowned::DrownedEntity, husk::HuskEntity, zombie::ZombieEntity};
use crate::entity::mob::zombified_piglin::ZombifiedPiglinEntity;
use crate::entity::passive::allay::AllayEntity;
use crate::entity::passive::armadillo::ArmadilloEntity;
use crate::entity::passive::axolotl::AxolotlEntity;
use crate::entity::passive::bee::BeeEntity;
use crate::entity::passive::camel::CamelEntity;
use crate::entity::passive::camel_husk::CamelHuskEntity;
use crate::entity::passive::cat::CatEntity;
use crate::entity::passive::chicken::ChickenEntity;
use crate::entity::passive::cod::CodEntity;
use crate::entity::passive::copper_golem::CopperGolemEntity;
use crate::entity::passive::cow::CowEntity;
use crate::entity::passive::dolphin::DolphinEntity;
use crate::entity::passive::donkey::DonkeyEntity;
use crate::entity::passive::fox::FoxEntity;
use crate::entity::passive::frog::FrogEntity;
use crate::entity::passive::glow_squid::GlowSquidEntity;
use crate::entity::passive::goat::GoatEntity;
use crate::entity::passive::happy_ghast::HappyGhastEntity;
use crate::entity::passive::horse::HorseEntity;
use crate::entity::passive::iron_golem::IronGolemEntity;
use crate::entity::passive::llama::LlamaEntity;
use crate::entity::passive::mooshroom::MooshroomEntity;
use crate::entity::passive::mule::MuleEntity;
use crate::entity::passive::nautilus::NautilusEntity;
use crate::entity::passive::zombie_nautilus::ZombieNautilusEntity;
use crate::entity::passive::ocelot::OcelotEntity;
use crate::entity::passive::panda::PandaEntity;
use crate::entity::passive::parrot::ParrotEntity;
use crate::entity::passive::pig::PigEntity;
use crate::entity::passive::polar_bear::PolarBearEntity;
use crate::entity::passive::pufferfish::PufferfishEntity;
use crate::entity::passive::rabbit::RabbitEntity;
use crate::entity::passive::salmon::SalmonEntity;
use crate::entity::passive::sheep::SheepEntity;
use crate::entity::passive::skeleton_horse::SkeletonHorseEntity;
use crate::entity::passive::sniffer::SnifferEntity;
use crate::entity::passive::snow_golem::SnowGolemEntity;
use crate::entity::passive::squid::SquidEntity;
use crate::entity::passive::strider::StriderEntity;
use crate::entity::passive::tadpole::TadpoleEntity;
use crate::entity::passive::trader_llama::TraderLlamaEntity;
use crate::entity::passive::tropical_fish::TropicalFishEntity;
use crate::entity::passive::turtle::TurtleEntity;
use crate::entity::passive::villager::VillagerEntity;
use crate::entity::passive::wandering_trader::WanderingTraderEntity;
use crate::entity::passive::wolf::WolfEntity;
use crate::entity::passive::zombie_horse::ZombieHorseEntity;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::arrow::ArrowEntity;
use crate::entity::projectile::dragon_fireball::DragonFireballEntity;
use crate::entity::projectile::egg::EggEntity;
use crate::entity::projectile::ender_pearl::EnderPearlEntity;
use crate::entity::projectile::experience_bottle::ExperienceBottleEntity;
use crate::entity::projectile::eye_of_ender::EyeOfEnder;
use crate::entity::projectile::fireball::FireballEntity;
use crate::entity::projectile::firework_rocket::FireworkRocketEntity;
use crate::entity::projectile::fishing_bobber::FishingBobberEntity;
use crate::entity::projectile::lingering_potion::LingeringPotionEntity;
use crate::entity::projectile::llama_spit::LlamaSpitEntity;
use crate::entity::projectile::shulker_bullet::ShulkerBulletEntity;
use crate::entity::projectile::small_fireball::SmallFireballEntity;
use crate::entity::projectile::snowball::SnowballEntity;
use crate::entity::projectile::splash_potion::SplashPotionEntity;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::entity::projectile::wither_skull::WitherSkullEntity;
use crate::entity::tnt::TNTEntity;
use crate::entity::vehicle::boat::BoatEntity;
use crate::entity::vehicle::minecart::MinecartEntity;
use crate::entity::{Entity, EntityBase, mob};
use crate::world::World;
use pumpkin_data::Block;
use std::sync::atomic::AtomicBool;

/// Builds the entity for `entity_type` at `position`.
#[expect(clippy::too_many_lines)]
pub fn from_type(
    entity_type: &'static EntityType,
    position: Vector3<f64>,
    world: &Arc<World>,
    uuid: Uuid,
) -> Arc<dyn EntityBase> {
    let entity = Entity::from_uuid(uuid, world.clone(), position, entity_type);

    let mob: Arc<dyn EntityBase> = match entity_type.id {
        // Zombie
        id if id == EntityType::ZOMBIE.id => ZombieEntity::new(entity),
        id if id == EntityType::DROWNED.id => DrownedEntity::new(entity),
        id if id == EntityType::HUSK.id => HuskEntity::new(entity),
        id if id == EntityType::ZOMBIE_VILLAGER.id => ZombieVillagerEntity::new(entity),
        id if id == EntityType::ZOMBIFIED_PIGLIN.id => ZombifiedPiglinEntity::new(entity),
        id if id == EntityType::ZOMBIE_NAUTILUS.id => ZombieNautilusEntity::new(entity),

        // Skeleton
        id if id == EntityType::SKELETON.id => SkeletonEntity::new(entity),
        id if id == EntityType::BOGGED.id => BoggedSkeletonEntity::new(entity),
        id if id == EntityType::PARCHED.id => ParchedSkeletonEntity::new(entity),
        id if id == EntityType::WITHER_SKELETON.id => WitherSkeletonEntity::new(entity),
        id if id == EntityType::STRAY.id => StraySkeletonEntity::new(entity),

        id if id == EntityType::BAT.id => BatEntity::new(entity),
        id if id == EntityType::CREEPER.id => CreeperEntity::new(entity),
        id if id == EntityType::ENDERMAN.id => EndermanEntity::new(entity),

        id if id == EntityType::BLAZE.id => BlazeEntity::new(entity),
        id if id == EntityType::SPIDER.id => SpiderEntity::new(entity),
        id if id == EntityType::CAVE_SPIDER.id => CaveSpiderEntity::new(entity),
        id if id == EntityType::GHAST.id => GhastEntity::new(entity),
        id if id == EntityType::MAGMA_CUBE.id => CubeMobEntity::new(entity, &MAGMA_CUBE),
        id if id == EntityType::SULFUR_CUBE.id => CubeMobEntity::new(entity, &SULFUR_CUBE),
        id if id == EntityType::PHANTOM.id => PhantomEntity::new(entity),
        id if id == EntityType::WITCH.id => WitchEntity::new(entity),
        id if id == EntityType::PIGLIN.id => PiglinEntity::new(entity),
        id if id == EntityType::PIGLIN_BRUTE.id => PiglinBruteEntity::new(entity),
        id if id == EntityType::PILLAGER.id => PillagerEntity::new(entity),
        id if id == EntityType::VINDICATOR.id => VindicatorEntity::new(entity),
        id if id == EntityType::EVOKER.id => EvokerEntity::new(entity),
        id if id == EntityType::RAVAGER.id => RavagerEntity::new(entity),
        id if id == EntityType::GUARDIAN.id => GuardianEntity::new(entity),
        id if id == EntityType::ELDER_GUARDIAN.id => ElderGuardianEntity::new(entity),
        id if id == EntityType::WARDEN.id => WardenEntity::new(entity),
        id if id == EntityType::HOGLIN.id => HoglinEntity::new(entity),
        id if id == EntityType::ZOGLIN.id => ZoglinEntity::new(entity),
        id if id == EntityType::BREEZE.id => BreezeEntity::new(entity),
        id if id == EntityType::CREAKING.id => CreakingEntity::new(entity),
        id if id == EntityType::ILLUSIONER.id => IllusionerEntity::new(entity),
        id if id == EntityType::VEX.id => VexEntity::new(entity),
        id if id == EntityType::ENDERMITE.id => EndermiteEntity::new(entity),
        id if id == EntityType::GIANT.id => GiantEntity::new(entity),

        id if id == EntityType::CAT.id => CatEntity::new(entity),
        id if id == EntityType::CHICKEN.id => ChickenEntity::new(entity),
        id if id == EntityType::COW.id => CowEntity::new(entity),
        id if id == EntityType::PIG.id => PigEntity::new(entity),
        id if id == EntityType::SHEEP.id => SheepEntity::new(entity),
        id if id == EntityType::WOLF.id => WolfEntity::new(entity),
        id if id == EntityType::FOX.id => FoxEntity::new(entity),
        id if id == EntityType::RABBIT.id => RabbitEntity::new(entity),
        id if id == EntityType::TURTLE.id => TurtleEntity::new(entity),
        id if id == EntityType::VILLAGER.id => VillagerEntity::new(entity),
        id if id == EntityType::SQUID.id => SquidEntity::new(entity),
        id if id == EntityType::HORSE.id => HorseEntity::new(entity),
        id if id == EntityType::DONKEY.id => DonkeyEntity::new(entity),
        id if id == EntityType::MULE.id => MuleEntity::new(entity),
        id if id == EntityType::ZOMBIE_HORSE.id => ZombieHorseEntity::new(entity),
        id if id == EntityType::SKELETON_HORSE.id => SkeletonHorseEntity::new(entity),
        id if id == EntityType::LLAMA.id => LlamaEntity::new(entity),
        id if id == EntityType::TRADER_LLAMA.id => TraderLlamaEntity::new(entity),
        id if id == EntityType::WANDERING_TRADER.id => WanderingTraderEntity::new(entity),
        id if id == EntityType::ALLAY.id => AllayEntity::new(entity),
        id if id == EntityType::ARMADILLO.id => ArmadilloEntity::new(entity),
        id if id == EntityType::AXOLOTL.id => AxolotlEntity::new(entity),
        id if id == EntityType::BEE.id => BeeEntity::new(entity),
        id if id == EntityType::CAMEL.id => CamelEntity::new(entity),
        id if id == EntityType::CAMEL_HUSK.id => CamelHuskEntity::new(entity),
        id if id == EntityType::COPPER_GOLEM.id => CopperGolemEntity::new(entity),
        id if id == EntityType::FROG.id => FrogEntity::new(entity),
        id if id == EntityType::GOAT.id => GoatEntity::new(entity),
        id if id == EntityType::HAPPY_GHAST.id => HappyGhastEntity::new(entity),
        id if id == EntityType::MOOSHROOM.id => MooshroomEntity::new(entity),
        id if id == EntityType::OCELOT.id => OcelotEntity::new(entity),
        id if id == EntityType::PANDA.id => PandaEntity::new(entity),
        id if id == EntityType::PARROT.id => ParrotEntity::new(entity),
        id if id == EntityType::POLAR_BEAR.id => PolarBearEntity::new(entity),
        id if id == EntityType::SNIFFER.id => SnifferEntity::new(entity),
        id if id == EntityType::STRIDER.id => StriderEntity::new(entity),
        id if id == EntityType::GLOW_SQUID.id => GlowSquidEntity::new(entity),
        id if id == EntityType::COD.id => CodEntity::new(entity),
        id if id == EntityType::SALMON.id => SalmonEntity::new(entity),
        id if id == EntityType::PUFFERFISH.id => PufferfishEntity::new(entity),
        id if id == EntityType::TROPICAL_FISH.id => TropicalFishEntity::new(entity),
        id if id == EntityType::TADPOLE.id => TadpoleEntity::new(entity),
        id if id == EntityType::DOLPHIN.id => DolphinEntity::new(entity),
        id if id == EntityType::NAUTILUS.id => NautilusEntity::new(entity),

        id if id == EntityType::SNOW_GOLEM.id => SnowGolemEntity::new(entity),
        id if id == EntityType::IRON_GOLEM.id => IronGolemEntity::new(entity),

        id if id == EntityType::WITHER.id => WitherEntity::new(entity),
        id if id == EntityType::ENDER_DRAGON.id => EnderDragonEntity::new(entity),

        id if id == EntityType::AREA_EFFECT_CLOUD.id => {
            crate::entity::area_effect_cloud::AreaEffectCloudEntity::new(entity)
        }
        id if id == EntityType::ARMOR_STAND.id => Arc::new(ArmorStandEntity::new(entity)),
        id if id == EntityType::CUSHION.id => Arc::new(CushionEntity::new(entity, 0)),
        id if id == EntityType::BLOCK_DISPLAY.id => BlockDisplayEntity::new(entity),
        id if id == EntityType::ITEM_DISPLAY.id => ItemDisplayEntity::new(entity),
        id if id == EntityType::TEXT_DISPLAY.id => TextDisplayEntity::new(entity),
        id if id == EntityType::PAINTING.id => Arc::new(PaintingEntity::new(entity)),
        id if id == EntityType::ITEM_FRAME.id || id == EntityType::GLOW_ITEM_FRAME.id => {
            Arc::new(ItemFrameEntity::new(entity))
        }
        id if id == EntityType::MANNEQUIN.id => Arc::new(MannequinEntity::new(entity)),
        id if id == EntityType::OMINOUS_ITEM_SPAWNER.id => {
            Arc::new(OminousItemSpawnerEntity::new(entity))
        }
        id if id == EntityType::EVOKER_FANGS.id => Arc::new(
            crate::entity::projectile::evoker_fangs::EvokerFangsEntity::new(entity, 0, 0.0, None),
        ),
        id if id == EntityType::END_CRYSTAL.id => Arc::new(EndCrystalEntity::new(entity)),
        id if id == EntityType::ENDER_PEARL.id => Arc::new(EnderPearlEntity::new(entity)),
        id if id == EntityType::SNOWBALL.id => Arc::new(SnowballEntity::new(entity)),
        id if id == EntityType::EGG.id => Arc::new(EggEntity::new(entity)),
        id if id == EntityType::SILVERFISH.id => SilverfishEntity::new(entity),
        id if id == EntityType::SLIME.id => CubeMobEntity::new(entity, &SLIME),
        id if id == EntityType::SHULKER.id => ShulkerEntity::new(entity),
        id if id == EntityType::SHULKER_BULLET.id => {
            // Shulker bullets are normally spawned by ShulkerEntity directly;
            // when loaded from the world we create a no-target bullet at the given position.
            Arc::new(ShulkerBulletEntity::orphan(entity))
        }
        id if id == EntityType::FALLING_BLOCK.id => {
            let falling = FallingEntity::new(entity, Block::SAND.default_state.id);
            falling.set_block_state(Block::SAND.default_state.id);
            Arc::new(falling)
        }
        id if id == EntityType::EXPERIENCE_ORB.id => Arc::new(ExperienceOrbEntity::new(entity, 1)),
        id if id == EntityType::TNT.id => Arc::new(TNTEntity::new(
            entity,
            TNTEntity::DEFAULT_POWER,
            TNTEntity::DEFAULT_FUSE,
        )),
        id if id == EntityType::ITEM.id => Arc::new(ItemEntity::new_empty(entity)),
        id if id == EntityType::ARROW.id => Arc::new(ArrowEntity::new(entity, None)),
        id if id == EntityType::SPECTRAL_ARROW.id => Arc::new(ArrowEntity::new(entity, None)),
        id if id == EntityType::TRIDENT.id => Arc::new(TridentEntity::new(entity, None)),
        id if id == EntityType::MINECART.id
            || id == EntityType::CHEST_MINECART.id
            || id == EntityType::FURNACE_MINECART.id
            || id == EntityType::TNT_MINECART.id
            || id == EntityType::HOPPER_MINECART.id
            || id == EntityType::COMMAND_BLOCK_MINECART.id
            || id == EntityType::SPAWNER_MINECART.id =>
        {
            Arc::new(MinecartEntity::new(entity))
        }
        id if id == EntityType::FIREBALL.id => Arc::new(FireballEntity::new(entity)),
        id if id == EntityType::SMALL_FIREBALL.id => Arc::new(SmallFireballEntity::new(entity)),
        id if id == EntityType::WITHER_SKULL.id => Arc::new(WitherSkullEntity::new(entity)),
        id if id == EntityType::WIND_CHARGE.id => {
            let thrown = ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                has_left_owner: AtomicBool::new(true),
                gravity: WIND_CHARGE_GRAVITY,
            };
            Arc::new(WindChargeEntity::new_normal(thrown))
        }
        id if id == EntityType::BREEZE_WIND_CHARGE.id => {
            let thrown = ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                has_left_owner: AtomicBool::new(true),
                gravity: WIND_CHARGE_GRAVITY,
            };
            Arc::new(WindChargeEntity::new_breeze(thrown))
        }
        id if id == EntityType::LIGHTNING_BOLT.id => Arc::new(LightningBoltEntity::new(entity)),
        id if id == EntityType::MARKER.id => MarkerEntity::new(entity),
        id if id == EntityType::LEASH_KNOT.id => Arc::new(
            crate::entity::decoration::leash_knot::LeashKnotEntity::new(entity),
        ),
        id if id == EntityType::INTERACTION.id => InteractionEntity::new(entity),
        id if id == EntityType::FIREWORK_ROCKET.id => {
            Arc::new(FireworkRocketEntity::new(entity))
        }
        id if id == EntityType::SPLASH_POTION.id => Arc::new(SplashPotionEntity::new(entity)),
        id if id == EntityType::EXPERIENCE_BOTTLE.id => {
            Arc::new(ExperienceBottleEntity::new(entity))
        }
        id if id == EntityType::LINGERING_POTION.id => Arc::new(LingeringPotionEntity::new(entity)),
        id if id == EntityType::EXPERIENCE_BOTTLE.id => {
            Arc::new(ExperienceBottleEntity::new(entity))
        }
        id if id == EntityType::DRAGON_FIREBALL.id => Arc::new(DragonFireballEntity::new(entity)),
        id if id == EntityType::FISHING_BOBBER.id => {
            Arc::new(FishingBobberEntity::new_unowned(entity))
        }
        id if id == EntityType::LLAMA_SPIT.id => Arc::new(LlamaSpitEntity::new(entity)),
        id if id == EntityType::EYE_OF_ENDER.id => Arc::new(EyeOfEnder::new(entity)),
        id if id == EntityType::ACACIA_BOAT.id
            || id == EntityType::ACACIA_CHEST_BOAT.id
            || id == EntityType::BIRCH_BOAT.id
            || id == EntityType::BIRCH_CHEST_BOAT.id
            || id == EntityType::DARK_OAK_BOAT.id
            || id == EntityType::DARK_OAK_CHEST_BOAT.id
            || id == EntityType::JUNGLE_BOAT.id
            || id == EntityType::JUNGLE_CHEST_BOAT.id
            || id == EntityType::MANGROVE_BOAT.id
            || id == EntityType::MANGROVE_CHEST_BOAT.id
            || id == EntityType::OAK_BOAT.id
            || id == EntityType::OAK_CHEST_BOAT.id
            || id == EntityType::PALE_OAK_BOAT.id
            || id == EntityType::PALE_OAK_CHEST_BOAT.id
            || id == EntityType::SPRUCE_BOAT.id
            || id == EntityType::SPRUCE_CHEST_BOAT.id
            || id == EntityType::BAMBOO_RAFT.id
            || id == EntityType::BAMBOO_CHEST_RAFT.id
            || id == EntityType::CHERRY_BOAT.id
            || id == EntityType::CHERRY_CHEST_BOAT.id
            || id == EntityType::POPLAR_BOAT.id
            || id == EntityType::POPLAR_CHEST_BOAT.id =>
        {
            Arc::new(BoatEntity::new(entity))
        }
        // Fallback Entity
        _ => {
            if entity_type.attributes.is_empty() {
                Arc::new(entity)
            } else {
                Arc::new(LivingEntity::new(entity))
            }
        }
    };

    if let (Some(living), Some(mob_impl)) = (mob.get_living_entity(), mob.get_mob()) {
        living.breath.air_supply.store(
            mob_impl.max_air_supply(),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    mob
}

#[expect(clippy::too_many_lines)]
pub fn check_spawn_rules(
    entity_type: &'static EntityType,
    world: &World,
    pos: &BlockPos,
    is_thundering: bool,
) -> bool {
    let id = entity_type.id;

    // Monsters (Standard Darkness + Peaceful check)
    if id == EntityType::BOGGED.id
        || id == EntityType::CAVE_SPIDER.id
        || id == EntityType::CREEPER.id
        || id == EntityType::ENDERMAN.id
        || id == EntityType::GIANT.id
        || id == EntityType::RAVAGER.id
        || id == EntityType::SKELETON.id
        || id == EntityType::SPIDER.id
        || id == EntityType::WITCH.id
        || id == EntityType::WITHER.id
        || id == EntityType::WITHER_SKELETON.id
        || id == EntityType::ZOMBIE.id
        || id == EntityType::ZOMBIE_HORSE.id
        || id == EntityType::ZOMBIE_VILLAGER.id
        || id == EntityType::CREAKING.id
        || id == EntityType::EVOKER.id
        || id == EntityType::ILLUSIONER.id
        || id == EntityType::VEX.id
        || id == EntityType::VINDICATOR.id
        || id == EntityType::WARDEN.id
    {
        return mob::MobEntity::check_monster_spawn_rules(world, pos, is_thundering);
    }

    // Any-light monsters (Blaze, Breeze, Zoglin)
    if id == EntityType::BLAZE.id || id == EntityType::BREEZE.id || id == EntityType::ZOGLIN.id {
        return mob::MobEntity::check_any_light_monster_spawn_rules(world, pos);
    }

    // Surface monsters (Husk, Parched, Camel Husk)
    if id == EntityType::HUSK.id || id == EntityType::PARCHED.id || id == EntityType::CAMEL_HUSK.id
    {
        return mob::MobEntity::check_surface_monsters_spawn_rules(world, pos, is_thundering);
    }

    // Stray
    if id == EntityType::STRAY.id {
        if !mob::MobEntity::check_monster_spawn_rules(world, pos, is_thundering) {
            return false;
        }
        let mut check_sky_pos = *pos;
        loop {
            check_sky_pos = check_sky_pos.up();
            if world.get_block(&check_sky_pos) != &pumpkin_data::Block::POWDER_SNOW {
                break;
            }
        }
        return world.can_see_sky(&check_sky_pos.down()) || world.can_see_sky(&check_sky_pos);
    }

    // Pillager (Patrolling Monster)
    if id == EntityType::PILLAGER.id {
        if world.get_block_light_level(pos).unwrap_or(0) > 8 {
            return false;
        }
        return mob::MobEntity::check_any_light_monster_spawn_rules(world, pos);
    }

    // Endermite & Silverfish
    if id == EntityType::ENDERMITE.id || id == EntityType::SILVERFISH.id {
        if !mob::MobEntity::check_any_light_monster_spawn_rules(world, pos) {
            return false;
        }
        return world
            .get_closest_player(pos.to_centered_f64(), 5.0)
            .is_none();
    }

    // Ghast
    if id == EntityType::GHAST.id {
        if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful {
            return false;
        }
        if rand::random_range(0..20) != 0 {
            return false;
        }
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }

    // Magma Cube
    if id == EntityType::MAGMA_CUBE.id {
        if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful {
            return false;
        }
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }

    // Sulfur Cube
    if id == EntityType::SULFUR_CUBE.id {
        // Vanilla SulfurCube.checkSulfurCubeSpawnRules always returns true.
        return true;
    }

    // Slime
    if id == EntityType::SLIME.id {
        return check_slime_spawn_rules(world, pos);
    }

    // Nether Mobs: Hoglin, Piglin, Zombified Piglin
    if id == EntityType::HOGLIN.id || id == EntityType::PIGLIN.id {
        let below = pos.down();
        if world.get_block(&below) == &pumpkin_data::Block::NETHER_WART_BLOCK {
            return false;
        }
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }
    if id == EntityType::ZOMBIFIED_PIGLIN.id {
        if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful {
            return false;
        }
        let below = pos.down();
        if world.get_block(&below) == &pumpkin_data::Block::NETHER_WART_BLOCK {
            return false;
        }
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }

    // Strider
    if id == EntityType::STRIDER.id {
        let mut check_pos = *pos;
        while world
            .get_fluid(&check_pos)
            .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_LAVA)
        {
            check_pos = check_pos.up();
        }
        return world.get_block(&check_pos).is_air();
    }

    // Bat
    if id == EntityType::BAT.id {
        return bat::BatEntity::check_bat_spawn_rules(world, pos);
    }

    // Animals (General)
    if id == EntityType::CHICKEN.id
        || id == EntityType::COW.id
        || id == EntityType::DONKEY.id
        || id == EntityType::HAPPY_GHAST.id
        || id == EntityType::HORSE.id
        || id == EntityType::LLAMA.id
        || id == EntityType::MULE.id
        || id == EntityType::PIG.id
        || id == EntityType::SHEEP.id
        || id == EntityType::CAT.id
        || id == EntityType::PANDA.id
        || id == EntityType::TRADER_LLAMA.id
    {
        return mob::MobEntity::check_animal_spawn_rules(world, pos);
    }

    // Tagged Animals: Armadillo, Camel, Frog, Goat, Mooshroom, Parrot, Rabbit, Wolf, Fox
    if id == EntityType::ARMADILLO.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_ARMADILLO_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::CAMEL.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_CAMELS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::FROG.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_FROGS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::GOAT.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_GOATS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::MOOSHROOM.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_MOOSHROOMS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::PARROT.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_PARROTS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::RABBIT.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_RABBITS_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::WOLF.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_WOLVES_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::FOX.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_FOXES_SPAWNABLE_ON)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::OCELOT.id {
        if rand::random_range(0..3) == 0 {
            return false;
        }
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }
    if id == EntityType::POLAR_BEAR.id {
        let below = pos.down();
        let biome = world.get_biome(pos);
        if biome.has_tag(
            &pumpkin_data::tag::WorldgenBiome::MINECRAFT_POLAR_BEARS_SPAWN_ON_ALTERNATE_BLOCKS,
        ) {
            return mob::MobEntity::is_bright_enough_to_spawn(world, pos)
                && world.get_block(&below).has_tag(
                    &pumpkin_data::tag::Block::MINECRAFT_POLAR_BEARS_SPAWNABLE_ON_ALTERNATE,
                );
        }
        return mob::MobEntity::check_animal_spawn_rules(world, pos);
    }
    if id == EntityType::TURTLE.id {
        let sea_level = world.sea_level;
        let below = pos.down();
        return pos.0.y < sea_level + 4
            && world
                .get_block(&below)
                .has_tag(&pumpkin_data::tag::Block::MINECRAFT_SAND)
            && mob::MobEntity::is_bright_enough_to_spawn(world, pos);
    }
    if id == EntityType::SKELETON_HORSE.id {
        return mob::MobEntity::check_animal_spawn_rules(world, pos);
    }

    // Water Animals / Creatures
    if id == EntityType::COD.id || id == EntityType::PUFFERFISH.id || id == EntityType::SALMON.id {
        return mob::MobEntity::check_surface_water_animal_spawn_rules(world, pos);
    }
    if id == EntityType::DOLPHIN.id || id == EntityType::SQUID.id {
        return mob::MobEntity::check_surface_ageable_water_creature_spawn_rules(world, pos);
    }
    if id == EntityType::TROPICAL_FISH.id {
        let biome = world.get_biome(pos);
        let valid_water = world
            .get_fluid(&pos.down())
            .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
            && (world.get_block(&pos.up()) == &pumpkin_data::Block::WATER
                || world
                    .get_fluid(&pos.up())
                    .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER));
        if !valid_water {
            return false;
        }
        return biome.has_tag(
            &pumpkin_data::tag::WorldgenBiome::MINECRAFT_ALLOWS_TROPICAL_FISH_SPAWNS_AT_ANY_HEIGHT,
        ) || mob::MobEntity::check_surface_water_animal_spawn_rules(world, pos);
    }
    if id == EntityType::AXOLOTL.id {
        let below = pos.down();
        return world
            .get_block(&below)
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_AXOLOTLS_SPAWNABLE_ON);
    }
    if id == EntityType::GLOW_SQUID.id {
        let sea_level = world.sea_level;
        return pos.0.y <= sea_level - 33
            && world.get_max_local_raw_brightness(pos) == 0
            && (world.get_block(pos) == &pumpkin_data::Block::WATER
                || world
                    .get_fluid(pos)
                    .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER));
    }
    if id == EntityType::NAUTILUS.id || id == EntityType::ZOMBIE_NAUTILUS.id {
        let sea_level = world.sea_level;
        let min_spawn_level = sea_level - 25;
        return pos.0.y >= min_spawn_level
            && pos.0.y <= sea_level - 5
            && world
                .get_fluid(&pos.down())
                .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
            && (world.get_block(&pos.up()) == &pumpkin_data::Block::WATER
                || world
                    .get_fluid(&pos.up())
                    .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER));
    }
    if id == EntityType::CAMEL_HUSK.id {
        return mob::MobEntity::check_surface_monsters_spawn_rules(world, pos, is_thundering);
    }
    if id == EntityType::DROWNED.id {
        if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful {
            return false;
        }
        if !world
            .get_fluid(&pos.down())
            .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
            || !world
                .get_fluid(pos)
                .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
        {
            return false;
        }
        if !mob::MobEntity::is_dark_enough_to_spawn(world, pos, is_thundering) {
            return false;
        }
        let biome = world.get_biome(pos);
        if biome.has_tag(&pumpkin_data::tag::WorldgenBiome::MINECRAFT_MORE_FREQUENT_DROWNED_SPAWNS)
        {
            return rand::random_range(0..15) == 0;
        }
        let sea_level = world.sea_level;
        return pos.0.y < sea_level - 5 && rand::random_range(0..40) == 0;
    }
    if id == EntityType::GUARDIAN.id || id == EntityType::ELDER_GUARDIAN.id {
        if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful {
            return false;
        }
        if !world
            .get_fluid(&pos.down())
            .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
            || !world
                .get_fluid(pos)
                .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER)
        {
            return false;
        }
        return rand::random_range(0..20) == 0 || !world.can_see_sky(pos);
    }

    // Any remaining Monster entities require standard monster spawn rules
    if entity_type.category == &MobCategory::MONSTER {
        return mob::MobEntity::check_monster_spawn_rules(world, pos, is_thundering);
    }

    // Generic mob spawn rules (Iron Golem, Snow Golem, Villager, Phantom, Shulker, Wandering Trader, Ender Dragon, etc.)
    if entity_type.mob {
        return mob::MobEntity::check_mob_spawn_rules(world, pos);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::mob::Mob;
    use arc_swap::ArcSwap;
    use pumpkin_config::world::LevelConfig;
    use pumpkin_data::attributes::Attributes;
    use pumpkin_data::damage::DamageType;
    use pumpkin_data::data_component_impl::{EquipmentSlot, PotionContentsImpl};
    use pumpkin_data::dimension::Dimension;
    use pumpkin_data::sound::Sound;
    use pumpkin_data::zombie_nautilus_variant::ZombieNautilusVariant;
    use pumpkin_world::{level::Level, world_info::LevelData};
    use std::sync::Weak;

    fn create_test_world() -> (Arc<World>, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let level = Level::from_root_folder(
            &LevelConfig::default(),
            directory.path().to_path_buf(),
            0,
            Dimension::OVERWORLD,
        );
        let world = Arc::new(World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(
                pumpkin_util::world_seed::Seed(0),
            ))),
            Dimension::OVERWORLD,
            crate::block::registry::default_registry(),
            Weak::new(),
        ));
        (world, directory)
    }

    #[tokio::test]
    async fn all_entity_types_from_type_constructible() {
        let (world, _dir) = create_test_world();
        let pos = Vector3::new(0.0, 64.0, 0.0);

        let mut tested = 0;
        for id in 0u16..161 {
            if let Some(entity_type) = EntityType::from_raw(id) {
                if entity_type == &EntityType::PLAYER {
                    continue;
                }
                let entity = from_type(entity_type, pos, &world, Uuid::new_v4());
                assert_eq!(entity.get_entity().entity_type, entity_type);
                tested += 1;
            }
        }
        assert_eq!(tested, 160, "Expected all 160 non-player entity types to be constructible");
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn camel_husk_parity_properties() {
        let (world, _dir) = create_test_world();
        let entity = from_type(&EntityType::CAMEL_HUSK, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        let mob = entity.get_mob().expect("CamelHusk should be a Mob");
        assert!(mob.remove_when_far_away(100.0), "CamelHusk should despawn like a monster");
        let sounds = mob.as_custom_sound().expect("CamelHusk should implement CustomSound");
        assert_eq!(sounds.death_sound(), Some(Sound::EntityCamelHuskDeath));
        assert_eq!(sounds.hurt_sound(), Some(Sound::EntityCamelHuskHurt));
        assert_eq!(mob.get_ambient_sound(), Some(Sound::EntityCamelHuskAmbient));
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn zombie_nautilus_parity_properties() {
        let (world, _dir) = create_test_world();
        let entity = from_type(&EntityType::ZOMBIE_NAUTILUS, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        let mob = entity.get_mob().expect("ZombieNautilus should be a Mob");
        assert!(mob.remove_when_far_away(100.0), "ZombieNautilus should despawn like a monster");
        assert!(matches!(mob.sun_protection_slot(), EquipmentSlot::Body(_)));
        let living = entity.get_living_entity().expect("ZombieNautilus should be LivingEntity");
        assert!((living.get_attribute_base(&Attributes::MOVEMENT_SPEED) - 1.1).abs() < 1e-4);
        let zn = entity.cast_any().downcast_ref::<ZombieNautilusEntity>().expect("downcast to ZombieNautilusEntity");
        assert_eq!(zn.get_variant(), ZombieNautilusVariant::Temperate);
        zn.set_variant(ZombieNautilusVariant::Warm);
        assert_eq!(zn.get_variant(), ZombieNautilusVariant::Warm);
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn parched_skeleton_parity_properties() {
        let (world, _dir) = create_test_world();
        let entity = from_type(&EntityType::PARCHED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        let mob = entity.get_mob().expect("Parched should be a Mob");
        assert_eq!(mob.get_ambient_sound(), Some(Sound::EntityParchedAmbient));
        let sounds = mob.as_custom_sound().expect("Parched should implement CustomSound");
        assert_eq!(sounds.death_sound(), Some(Sound::EntityParchedDeath));
        assert_eq!(sounds.hurt_sound(), Some(Sound::EntityParchedHurt));
        let living = entity.get_living_entity().expect("Parched should be LivingEntity");
        assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
        let arrow = mob.get_arrow_projectile().expect("Parched should shoot custom arrow");
        let potion = arrow.get_data_component::<PotionContentsImpl>().expect("Arrow has PotionContentsImpl");
        assert_eq!(potion.custom_effects[0].effect_id, "minecraft:weakness");
        assert_eq!(potion.custom_effects[0].duration, 600);
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn breeze_projectile_deflection() {
        let (world, _dir) = create_test_world();
        let breeze = from_type(&EntityType::BREEZE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        let arrow = from_type(&EntityType::ARROW, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        arrow.get_entity().velocity.store(Vector3::new(1.0, 0.0, 0.0));

        let mob = breeze.get_mob().expect("Breeze should be a Mob");
        let damaged = mob.pre_damage(DamageType::ARROW, Some(arrow.as_ref()));
        assert!(!damaged, "Breeze should deflect projectile damage");

        let velo = arrow.get_entity().velocity.load();
        assert!((velo.x - (-0.5)).abs() < 1e-4, "Projectile velocity should be reversed by -0.5");
        assert!(arrow.get_entity().velocity_dirty.load(std::sync::atomic::Ordering::Relaxed));
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn creaking_pushable_parity() {
        let (world, _dir) = create_test_world();
        let creaking = from_type(&EntityType::CREAKING, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        assert!(creaking.is_pushable());
        let creaking_entity = creaking.cast_any().downcast_ref::<CreakingEntity>().expect("downcast to CreakingEntity");
        creaking_entity.set_can_move(false);
        assert!(!creaking.is_pushable());
        creaking_entity.set_can_move(true);
        assert!(creaking.is_pushable());
        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn jump_control_lifecycle_and_tick_parity() {
        let (world, _dir) = create_test_world();
        let entity = from_type(&EntityType::CREEPER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        let mob = entity.get_mob().expect("Creeper should be a Mob");
        let mob_entity = mob.get_mob_entity();

        // 1. Initial state: jumping is false
        assert!(!mob_entity.living_entity.jumping.load(std::sync::atomic::Ordering::SeqCst));

        // 2. Request jump via helper (vanilla Mob.getJumpControl().jump())
        mob_entity.jump();

        // 3. Tick jump_control: transfers jump request to jumping flag and resets internal state
        {
            let mut jump_ctrl = mob_entity.jump_control.lock().unwrap();
            jump_ctrl.tick(mob);
        }
        assert!(mob_entity.living_entity.jumping.load(std::sync::atomic::Ordering::SeqCst));

        // 4. Tick again without calling jump(): jumping flag becomes false
        {
            let mut jump_ctrl = mob_entity.jump_control.lock().unwrap();
            jump_ctrl.tick(mob);
        }
        assert!(!mob_entity.living_entity.jumping.load(std::sync::atomic::Ordering::SeqCst));

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn land_mobs_goal_selector_stroll_parity() {
        let (world, _dir) = create_test_world();
        let mob_types = [
            &EntityType::CREEPER,
            &EntityType::ZOMBIE,
            &EntityType::SKELETON,
            &EntityType::SPIDER,
            &EntityType::ENDERMAN,
            &EntityType::COW,
            &EntityType::SHEEP,
            &EntityType::PIG,
            &EntityType::CHICKEN,
            &EntityType::WOLF,
        ];

        for &mob_type in &mob_types {
            let entity = from_type(mob_type, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = entity.get_mob().expect("Entity should be a Mob");
            let mob_entity = mob.get_mob_entity();

            // Verify GoalSelector and TargetSelector can be locked and ticked without error
            let mut goals = mob_entity.goals_selector.lock().unwrap();
            goals.tick(mob);
            assert!(goals.get_available_goals().len() > 0, "Mob {:?} must have goals registered", mob_type);

            let mut targets = mob_entity.target_selector.lock().unwrap();
            targets.tick(mob);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn avoid_and_target_goal_selector_parity() {
        let (world, _dir) = create_test_world();
        let test_types = [
            &EntityType::SPIDER,
            &EntityType::CAVE_SPIDER,
            &EntityType::PILLAGER,
            &EntityType::VINDICATOR,
            &EntityType::EVOKER,
            &EntityType::RAVAGER,
            &EntityType::ZOMBIE,
            &EntityType::DROWNED,
        ];

        for &mob_type in &test_types {
            let entity = from_type(mob_type, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = entity.get_mob().expect("Entity should be a Mob");
            let mob_entity = mob.get_mob_entity();

            let mut goals = mob_entity.goals_selector.lock().unwrap();
            goals.tick(mob);
            assert!(
                !goals.get_available_goals().is_empty(),
                "Mob {:?} must have registered goals",
                mob_type
            );

            let mut targets = mob_entity.target_selector.lock().unwrap();
            targets.tick(mob);
            assert!(
                !targets.get_available_goals().is_empty(),
                "Mob {:?} must have registered targets",
                mob_type
            );
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn updated_mobs_goals_and_targets_parity() {
        let (world, _dir) = create_test_world();
        let updated_mobs = [
            &EntityType::VILLAGER,
            &EntityType::WARDEN,
            &EntityType::BREEZE,
            &EntityType::HOGLIN,
            &EntityType::ZOGLIN,
            &EntityType::ZOMBIFIED_PIGLIN,
            &EntityType::IRON_GOLEM,
            &EntityType::SNOW_GOLEM,
            &EntityType::RABBIT,
            &EntityType::FROG,
            &EntityType::GOAT,
            &EntityType::PANDA,
            &EntityType::CAMEL,
            &EntityType::CAMEL_HUSK,
            &EntityType::SNIFFER,
            &EntityType::ARMADILLO,
            &EntityType::CREAKING,
        ];

        for &mob_type in &updated_mobs {
            let entity = from_type(mob_type, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = entity.get_mob().expect("Entity should be a Mob");
            let mob_entity = mob.get_mob_entity();

            let mut goals = mob_entity.goals_selector.lock().unwrap();
            goals.tick(mob);
            assert!(
                !goals.get_available_goals().is_empty(),
                "Mob {:?} must have registered goals",
                mob_type
            );

            let mut targets = mob_entity.target_selector.lock().unwrap();
            targets.tick(mob);
        }

        // Specific checks for key parity fixes
        {
            // Breeze has RevengeGoal + Player + IronGolem targets
            let breeze = from_type(&EntityType::BREEZE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let targets = breeze.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 3, "Breeze should have at least 3 target goals (Revenge, Player, Golem)");
        }

        {
            // Villager has multiple threat avoidances + stroll + look
            let villager = from_type(&EntityType::VILLAGER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let goals = villager.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 12, "Villager should have at least 12 goals for threats and stroll");
        }

        {
            // Snow Golem has Monster target goal
            let snow_golem = from_type(&EntityType::SNOW_GOLEM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let targets = snow_golem.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(!targets.get_available_goals().is_empty(), "Snow Golem must have monster target goal");
        }

        {
            // Rabbit has avoid monsters + flee players/wolves/foxes
            let rabbit = from_type(&EntityType::RABBIT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let goals = rabbit.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 5, "Rabbit should have panic, breed, tempt, avoid threats, stroll");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn avoid_entity_goal_monster_predicate() {
        use crate::entity::ai::goal::avoid_entity::AvoidEntityGoal;
        use pumpkin_data::entity::MobCategory;

        let avoid_monster_goal = AvoidEntityGoal::for_monsters(4.0, 2.2, 2.2);
        assert!(avoid_monster_goal.flee_type().is_none(), "Monster avoid goal should use dynamic predicate");

        let (world, _dir) = create_test_world();
        let zombie = from_type(&EntityType::ZOMBIE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        assert_eq!(zombie.get_entity().entity_type.category, &MobCategory::MONSTER);

        let sheep = from_type(&EntityType::SHEEP, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
        assert_ne!(sheep.get_entity().entity_type.category, &MobCategory::MONSTER);

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn raiders_and_skeletons_and_felines_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Bogged: MAX_HEALTH = 16.0, shoots poison arrow
        {
            let bogged = from_type(&EntityType::BOGGED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = bogged.get_living_entity().expect("Bogged is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            let mob = bogged.get_mob().expect("Bogged is mob");
            let arrow = mob.get_arrow_projectile().expect("Bogged shoots custom arrow");
            let potion = arrow.get_data_component::<PotionContentsImpl>().expect("Arrow has potion component");
            assert_eq!(potion.custom_effects[0].effect_id, "minecraft:poison");
            assert_eq!(potion.custom_effects[0].duration, 100);
        }

        // 2. Stray: shoots slowness arrow (duration 600)
        {
            let stray = from_type(&EntityType::STRAY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = stray.get_mob().expect("Stray is mob");
            let arrow = mob.get_arrow_projectile().expect("Stray shoots custom arrow");
            let potion = arrow.get_data_component::<PotionContentsImpl>().expect("Arrow has potion component");
            assert_eq!(potion.custom_effects[0].effect_id, "minecraft:slowness");
            assert_eq!(potion.custom_effects[0].duration, 600);
        }

        // 3. Vex: MAX_HEALTH = 14.0, ATTACK_DAMAGE = 4.0, has RevengeGoal
        {
            let vex = from_type(&EntityType::VEX, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = vex.get_living_entity().expect("Vex is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 4.0);
            let targets = vex.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 2, "Vex must have revenge and player targets");
        }

        // 4. Illusioner: has BowAttackGoal, Avoid Creaking, RevengeGoal
        {
            let illusioner = from_type(&EntityType::ILLUSIONER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let goals = illusioner.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 8, "Illusioner must have bow attack, avoid, patrol, and stroll");
            let targets = illusioner.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 4, "Illusioner must have revenge, player, villager, and golem targets");
        }

        // 5. Fox: has avoid goals (Player, Wolf, PolarBear)
        {
            let fox = from_type(&EntityType::FOX, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let goals = fox.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 10, "Fox must have panic, breed, tempt, avoid threats, follow, stroll");
        }

        // 6. TraderLlama: defends against zombies and illagers
        {
            let trader_llama = from_type(&EntityType::TRADER_LLAMA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let targets = trader_llama.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 8, "Trader Llama must defend against wolves, zombies, and illagers");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn aquatic_equine_bosses_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Axolotl: MAX_HEALTH = 14.0, MOVEMENT_SPEED = 1.0, ATTACK_DAMAGE = 2.0, STEP_HEIGHT = 1.0
        {
            let axolotl = from_type(&EntityType::AXOLOTL, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = axolotl.get_living_entity().expect("Axolotl is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            let targets = axolotl.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 9, "Axolotl must target hostiles, fish, squid, pufferfish, and tadpoles");
        }

        // 2. Turtle: MAX_HEALTH = 30.0, MOVEMENT_SPEED = 0.25, ambient interval = 200
        {
            let turtle = from_type(&EntityType::TURTLE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = turtle.get_living_entity().expect("Turtle is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            let mob = turtle.get_mob().expect("Turtle is mob");
            assert_eq!(mob.get_ambient_sound_interval(), 200);
            let goals = mob.get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 6, "Turtle must have panic, breed, tempt, go to water, look at player, and stroll");
        }

        // 3. Dolphin: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 1.2, ATTACK_DAMAGE = 3.0
        {
            let dolphin = from_type(&EntityType::DOLPHIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = dolphin.get_living_entity().expect("Dolphin is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.2);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            let goals = dolphin.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 7, "Dolphin must have swim, avoid, attack, tempt, and look goals");
        }

        // 4. Donkey & Mule: MOVEMENT_SPEED = 0.175, JUMP_STRENGTH = 0.5
        {
            let donkey = from_type(&EntityType::DONKEY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = donkey.get_living_entity().expect("Donkey is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);

            let mule = from_type(&EntityType::MULE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = mule.get_living_entity().expect("Mule is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);
        }

        // 5. SkeletonHorse & ZombieHorse: parity attributes
        {
            let skel_horse = from_type(&EntityType::SKELETON_HORSE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = skel_horse.get_living_entity().expect("SkeletonHorse is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 15.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);

            let zomb_horse = from_type(&EntityType::ZOMBIE_HORSE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = zomb_horse.get_living_entity().expect("ZombieHorse is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 25.0);
        }

        // 6. PolarBear: MAX_HEALTH = 30.0, FOLLOW_RANGE = 20.0, MOVEMENT_SPEED = 0.25, ATTACK_DAMAGE = 6.0
        {
            let polar_bear = from_type(&EntityType::POLAR_BEAR, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = polar_bear.get_living_entity().expect("PolarBear is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
        }

        // 7. Wither: MAX_HEALTH = 300.0, MOVEMENT_SPEED = 0.6, FLYING_SPEED = 0.6, FOLLOW_RANGE = 40.0, ARMOR = 4.0
        {
            let wither = from_type(&EntityType::WITHER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = wither.get_living_entity().expect("Wither is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 300.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 4.0);
            let targets = wither.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 2, "Wither must have revenge and living entity target goals");
        }

        // 8. EnderDragon: MAX_HEALTH = 200.0
        {
            let dragon = from_type(&EntityType::ENDER_DRAGON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = dragon.get_living_entity().expect("EnderDragon is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 200.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn allay_bee_ravager_shulker_strider_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Allay: MAX_HEALTH = 20.0, FLYING_SPEED = 0.1, MOVEMENT_SPEED = 0.1, ATTACK_DAMAGE = 2.0
        {
            let allay = from_type(&EntityType::ALLAY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = allay.get_living_entity().expect("Allay is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.1);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.1);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 20.0);
            let goals = allay.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 4, "Allay must have registered goals");
        }

        // 2. Bee: MAX_HEALTH = 10.0, FLYING_SPEED = 0.6, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 2.0
        {
            let bee = from_type(&EntityType::BEE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = bee.get_living_entity().expect("Bee is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 10.0);
            let targets = bee.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert!(targets.get_available_goals().len() >= 3, "Bee must have revenge and anger target goals");
        }

        // 3. Ravager: MAX_HEALTH = 100.0, MOVEMENT_SPEED = 0.3, KNOCKBACK_RESISTANCE = 0.75, ATTACK_DAMAGE = 12.0, ATTACK_KNOCKBACK = 1.5, FOLLOW_RANGE = 32.0, STEP_HEIGHT = 1.0
        {
            let ravager = from_type(&EntityType::RAVAGER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = ravager.get_living_entity().expect("Ravager is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 100.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.75);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 12.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_KNOCKBACK), 1.5);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 32.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 100.0);
        }

        // 4. Shulker: MAX_HEALTH = 30.0
        {
            let shulker = from_type(&EntityType::SHULKER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = shulker.get_living_entity().expect("Shulker is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.health.load(), 30.0);
        }

        // 5. Strider: MAX_HEALTH = 20.0, MOVEMENT_SPEED = 0.175
        {
            let strider = from_type(&EntityType::STRIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = strider.get_living_entity().expect("Strider is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.health.load(), 20.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn cat_parrot_mooshroom_silverfish_endermite_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Cat: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 3.0, ambient_sound_interval = 120
        {
            let cat = from_type(&EntityType::CAT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = cat.get_living_entity().expect("Cat is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.health.load(), 10.0);
            let mob = cat.get_mob().expect("Cat is mob");
            assert_eq!(mob.get_ambient_sound_interval(), 120);
            let goals = mob.get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 10, "Cat must have vanilla goals");
            let targets = mob.get_mob_entity().target_selector.lock().unwrap();
            assert_eq!(targets.get_available_goals().len(), 2, "Cat must have 2 untamed prey target goals");
        }

        // 2. Parrot: MAX_HEALTH = 6.0, FLYING_SPEED = 0.4, MOVEMENT_SPEED = 0.2, ATTACK_DAMAGE = 3.0
        {
            let parrot = from_type(&EntityType::PARROT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = parrot.get_living_entity().expect("Parrot is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.4);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.health.load(), 6.0);
            let goals = parrot.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 3, "Parrot must have panic, float, look, wander goals");
        }

        // 3. Mooshroom: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.2
        {
            let mooshroom = from_type(&EntityType::MOOSHROOM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = mooshroom.get_living_entity().expect("Mooshroom is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.health.load(), 10.0);
            let goals = mooshroom.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 7, "Mooshroom must have full cow goal set");
        }

        // 4. Silverfish: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.25, ATTACK_DAMAGE = 1.0
        {
            let silverfish = from_type(&EntityType::SILVERFISH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = silverfish.get_living_entity().expect("Silverfish is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 1.0);
            assert_eq!(living.health.load(), 8.0);
            let targets = silverfish.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert_eq!(targets.get_available_goals().len(), 2, "Silverfish must have revenge and player target goals");
        }

        // 5. Endermite: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.25, ATTACK_DAMAGE = 2.0
        {
            let endermite = from_type(&EntityType::ENDERMITE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = endermite.get_living_entity().expect("Endermite is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 8.0);
            let goals = endermite.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert_eq!(goals.get_available_goals().len(), 6, "Endermite must have 6 vanilla goals");
            let targets = endermite.get_mob().unwrap().get_mob_entity().target_selector.lock().unwrap();
            assert_eq!(targets.get_available_goals().len(), 2, "Endermite must have revenge and player target goals");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn cave_spider_giant_pufferfish_wandering_trader_piglin_brute_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. CaveSpider: MAX_HEALTH = 12.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 2.0
        {
            let spider = from_type(&EntityType::CAVE_SPIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = spider.get_living_entity().expect("CaveSpider is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 12.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 12.0);
            let mob = spider.get_mob().unwrap();
            let goals = mob.get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 6, "CaveSpider must have vanilla goals");
            let targets = mob.get_mob_entity().target_selector.lock().unwrap();
            assert_eq!(targets.get_available_goals().len(), 3, "CaveSpider must have revenge, player and golem targets");
        }

        // 2. Giant: MAX_HEALTH = 100.0, MOVEMENT_SPEED = 0.5, ATTACK_DAMAGE = 50.0, CAMERA_DISTANCE = 16.0
        {
            let giant = from_type(&EntityType::GIANT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = giant.get_living_entity().expect("Giant is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 100.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.5);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 50.0);
            assert_eq!(living.get_attribute_base(&Attributes::CAMERA_DISTANCE), 16.0);
            assert_eq!(living.health.load(), 100.0);
        }

        // 3. Pufferfish: MAX_HEALTH = 3.0
        {
            let pufferfish = from_type(&EntityType::PUFFERFISH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = pufferfish.get_living_entity().expect("Pufferfish is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 3.0);
            assert_eq!(living.health.load(), 3.0);
            let goals = pufferfish.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 4, "Pufferfish must have panic, puff, avoid player and swim goals");
        }

        // 4. WanderingTrader: MAX_HEALTH = 20.0, MOVEMENT_SPEED = 0.5
        {
            let trader = from_type(&EntityType::WANDERING_TRADER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = trader.get_living_entity().expect("WanderingTrader is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.5);
            assert_eq!(living.health.load(), 20.0);
            let goals = trader.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 10, "WanderingTrader must have full goal set");
        }

        // 5. PiglinBrute: MAX_HEALTH = 50.0, MOVEMENT_SPEED = 0.35, ATTACK_DAMAGE = 7.0, FOLLOW_RANGE = 12.0
        {
            let brute = from_type(&EntityType::PIGLIN_BRUTE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = brute.get_living_entity().expect("PiglinBrute is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 50.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.35);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 7.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 12.0);
            assert_eq!(living.health.load(), 50.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn aquatic_and_passive_mobs_parity_tests() {
        let (world, _temp_dir) = create_test_world();

        // 1. Cod: MAX_HEALTH = 3.0, goals 0, 2, 4
        {
            let cod = from_type(&EntityType::COD, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = cod.get_living_entity().expect("Cod is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 3.0);
            assert_eq!(living.health.load(), 3.0);
            let goals = cod.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            let available = goals.get_available_goals();
            assert_eq!(available.len(), 3, "Cod has exactly panic, avoid, and swim goals");
            assert_eq!(available[0].priority, 0);
            assert_eq!(available[1].priority, 2);
            assert_eq!(available[2].priority, 4);
        }

        // 2. Salmon: MAX_HEALTH = 3.0, goals 0, 2, 4
        {
            let salmon = from_type(&EntityType::SALMON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = salmon.get_living_entity().expect("Salmon is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 3.0);
            assert_eq!(living.health.load(), 3.0);
            let goals = salmon.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            let available = goals.get_available_goals();
            assert_eq!(available.len(), 3, "Salmon has exactly panic, avoid, and swim goals");
            assert_eq!(available[0].priority, 0);
            assert_eq!(available[1].priority, 2);
            assert_eq!(available[2].priority, 4);
        }

        // 3. TropicalFish: MAX_HEALTH = 3.0, goals 0, 2, 4
        {
            let tropical_fish = from_type(&EntityType::TROPICAL_FISH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = tropical_fish.get_living_entity().expect("TropicalFish is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 3.0);
            assert_eq!(living.health.load(), 3.0);
            let goals = tropical_fish.get_mob().unwrap().get_mob_entity().goals_selector.lock().unwrap();
            let available = goals.get_available_goals();
            assert_eq!(available.len(), 3, "TropicalFish has exactly panic, avoid, and swim goals");
            assert_eq!(available[0].priority, 0);
            assert_eq!(available[1].priority, 2);
            assert_eq!(available[2].priority, 4);
        }

        // 4. Tadpole: MAX_HEALTH = 6.0, MOVEMENT_SPEED = 1.0
        {
            let tadpole = from_type(&EntityType::TADPOLE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = tadpole.get_living_entity().expect("Tadpole is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.0);
            assert_eq!(living.health.load(), 6.0);
        }

        // 5. Squid: MAX_HEALTH = 10.0
        {
            let squid = from_type(&EntityType::SQUID, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = squid.get_living_entity().expect("Squid is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.health.load(), 10.0);
        }

        // 6. GlowSquid: MAX_HEALTH = 10.0
        {
            let glow_squid = from_type(&EntityType::GLOW_SQUID, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = glow_squid.get_living_entity().expect("GlowSquid is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.health.load(), 10.0);
        }

        // 7. Ocelot: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 3.0
        {
            let ocelot = from_type(&EntityType::OCELOT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = ocelot.get_living_entity().expect("Ocelot is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.health.load(), 10.0);
        }

        // 8. Chicken: MAX_HEALTH = 4.0, MOVEMENT_SPEED = 0.25
        {
            let chicken = from_type(&EntityType::CHICKEN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = chicken.get_living_entity().expect("Chicken is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 4.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 4.0);
        }

        // 9. Cow: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.2
        {
            let cow = from_type(&EntityType::COW, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = cow.get_living_entity().expect("Cow is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.health.load(), 10.0);
        }

        // 10. Pig: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.25
        {
            let pig = from_type(&EntityType::PIG, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = pig.get_living_entity().expect("Pig is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 10.0);
        }

        // 11. Sheep: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.23
        {
            let sheep = from_type(&EntityType::SHEEP, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = sheep.get_living_entity().expect("Sheep is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.health.load(), 8.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn guardian_and_elder_guardian_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Guardian: MAX_HEALTH = 30.0, MOVEMENT_SPEED = 0.5, ATTACK_DAMAGE = 6.0
        {
            let guardian = from_type(&EntityType::GUARDIAN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = guardian.get_living_entity().expect("Guardian is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.5);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 30.0);
        }

        // 2. Elder Guardian: MAX_HEALTH = 80.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 8.0, persistence_required = true
        {
            let elder = from_type(&EntityType::ELDER_GUARDIAN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = elder.get_living_entity().expect("Elder Guardian is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 80.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 8.0);
            assert_eq!(living.health.load(), 80.0);
            let mob = elder.get_mob().expect("Elder Guardian is mob entity");
            assert!(mob.get_mob_entity().persistence_required.load(std::sync::atomic::Ordering::Relaxed));
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn hostile_mobs_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Witch: MAX_HEALTH = 26.0, MOVEMENT_SPEED = 0.25
        {
            let witch = from_type(&EntityType::WITCH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = witch.get_living_entity().expect("Witch is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 26.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 26.0);
        }

        // 2. Blaze: MAX_HEALTH = 20.0, ATTACK_DAMAGE = 6.0, MOVEMENT_SPEED = 0.23, FOLLOW_RANGE = 48.0
        {
            let blaze = from_type(&EntityType::BLAZE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = blaze.get_living_entity().expect("Blaze is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 48.0);
            assert_eq!(living.health.load(), 20.0);
        }

        // 3. Ghast: MAX_HEALTH = 10.0, FOLLOW_RANGE = 100.0, CAMERA_DISTANCE = 8.0, FLYING_SPEED = 0.06
        {
            let ghast = from_type(&EntityType::GHAST, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = ghast.get_living_entity().expect("Ghast is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 100.0);
            assert_eq!(living.get_attribute_base(&Attributes::CAMERA_DISTANCE), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.06);
            assert_eq!(living.health.load(), 10.0);
        }

        // 4. Phantom: MAX_HEALTH = 20.0, ATTACK_DAMAGE = 6.0 (at size 0), size scaling
        {
            let phantom = from_type(&EntityType::PHANTOM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = phantom.get_living_entity().expect("Phantom is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 20.0);

            let phantom_entity = phantom
                .cast_any()
                .downcast_ref::<crate::entity::mob::phantom::PhantomEntity>()
                .expect("downcast to PhantomEntity");
            phantom_entity.set_phantom_size(2);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 8.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn classic_monsters_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Creeper: MAX_HEALTH = 20.0, MOVEMENT_SPEED = 0.25, fall swell
        {
            let creeper = from_type(&EntityType::CREEPER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = creeper.get_living_entity().expect("Creeper is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 20.0);

            let creeper_entity = creeper
                .cast_any()
                .downcast_ref::<crate::entity::mob::creeper::CreeperEntity>()
                .expect("downcast to CreeperEntity");
            living.fall_distance.store(4.0);
            let mob = creeper.get_mob().expect("Creeper is Mob");
            mob.on_damage(DamageType::FALL, None);
            assert_eq!(creeper_entity.current_fuse_time.load(std::sync::atomic::Ordering::Relaxed), 6);
        }

        // 2. Enderman: MAX_HEALTH = 40.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 7.0, FOLLOW_RANGE = 64.0, STEP_HEIGHT = 1.0
        {
            let enderman = from_type(&EntityType::ENDERMAN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = enderman.get_living_entity().expect("Enderman is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 7.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 64.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 40.0);
        }

        // 3. Spider: MAX_HEALTH = 16.0, ATTACK_DAMAGE = 2.0, MOVEMENT_SPEED = 0.3
        {
            let spider = from_type(&EntityType::SPIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = spider.get_living_entity().expect("Spider is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.health.load(), 16.0);
        }

        // 4. Bat: MAX_HEALTH = 6.0, collides = false
        {
            let bat = from_type(&EntityType::BAT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = bat.get_living_entity().expect("Bat is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 6.0);
            assert_eq!(living.health.load(), 6.0);
            assert!(!living.collides.load(std::sync::atomic::Ordering::Relaxed));
        }

        // 5. Zombie: MAX_HEALTH = 20.0, FOLLOW_RANGE = 35.0, MOVEMENT_SPEED = 0.23, ATTACK_DAMAGE = 3.0, ARMOR = 2.0
        {
            let zombie = from_type(&EntityType::ZOMBIE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = zombie.get_living_entity().expect("Zombie is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 35.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);
            assert_eq!(living.health.load(), 20.0);
        }

        // 6. Drowned: STEP_HEIGHT = 1.0
        {
            let drowned = from_type(&EntityType::DROWNED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = drowned.get_living_entity().expect("Drowned is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 20.0);
        }

        // 7. Zombified Piglin: ATTACK_DAMAGE = 5.0, ARMOR = 2.0
        {
            let zombified = from_type(&EntityType::ZOMBIFIED_PIGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = zombified.get_living_entity().expect("Zombified Piglin is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 5.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);
            assert_eq!(living.health.load(), 20.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn skeletons_vex_allay_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Skeleton: min attack interval is 40 on Normal/Easy/Peaceful, 20 on Hard
        {
            let skeleton = from_type(&EntityType::SKELETON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = skeleton.get_mob().expect("Skeleton is mob");
            assert_eq!(mob.get_min_attack_interval(), Some(40), "Default world difficulty normal/easy should give 40 ticks");
            let living = skeleton.get_living_entity().expect("Skeleton is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
        }

        // 2. Stray: min attack interval is 40 on Normal/Easy/Peaceful, 20 on Hard
        {
            let stray = from_type(&EntityType::STRAY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = stray.get_mob().expect("Stray is mob");
            assert_eq!(mob.get_min_attack_interval(), Some(40), "Default world difficulty should give 40 ticks");
            let arrow = mob.get_arrow_projectile().expect("Stray has arrow projectile");
            assert_eq!(arrow.item.id, pumpkin_data::item::Item::ARROW.id);
        }

        // 3. Vex: no_physics = true, has_no_gravity = true, burns_in_daylight = false, health = 14, damage = 4
        {
            let vex = from_type(&EntityType::VEX, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let entity = vex.get_entity();
            assert!(entity.no_physics.load(std::sync::atomic::Ordering::Relaxed), "Vex must fly through blocks with no_physics");
            assert!(entity.has_no_gravity(), "Vex must have no gravity");
            let mob = vex.get_mob().expect("Vex is mob");
            assert!(!mob.burns_in_daylight(), "Vex does not burn in daylight");
            let living = vex.get_living_entity().expect("Vex is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 4.0);
        }

        // 4. Allay: MAX_HEALTH = 20.0, FLYING_SPEED = 0.1, MOVEMENT_SPEED = 0.1, ATTACK_DAMAGE = 2.0
        {
            let allay = from_type(&EntityType::ALLAY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let living = allay.get_living_entity().expect("Allay is living entity");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.1);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.1);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            let mob = allay.get_mob().expect("Allay is mob");
            assert_eq!(mob.get_ambient_sound(), Some(pumpkin_data::sound::Sound::EntityAllayAmbientWithoutItem));
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn skeleton_powder_snow_and_zombie_water_conversion_tests() {
        let (world, _dir) = create_test_world();

        // 1. Skeleton Stray conversion tracker
        {
            let skeleton = from_type(&EntityType::SKELETON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let skeleton_mob = skeleton.cast_any().downcast_ref::<crate::entity::mob::skeleton::skeleton::SkeletonEntity>()
                .expect("Skeleton downcast");
            assert!(!skeleton_mob.is_converting(), "Skeleton must not be converting initially");
            skeleton_mob.start_freeze_conversion(300);
            assert!(skeleton_mob.is_converting(), "Skeleton must be converting after start_freeze_conversion");
            let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
            skeleton_mob.mob_write_nbt(&mut nbt);
            assert_eq!(nbt.get_int("StrayConversionTime"), Some(300));
        }

        // 2. Zombie Drowned conversion tracker
        {
            let zombie = from_type(&EntityType::ZOMBIE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let zombie_mob = zombie.cast_any().downcast_ref::<crate::entity::mob::zombie::zombie::ZombieEntity>()
                .expect("Zombie downcast");
            assert!(!zombie_mob.is_converting(), "Zombie must not be converting initially");
            zombie_mob.start_drowned_conversion(300);
            assert!(zombie_mob.is_converting(), "Zombie must be converting after start_drowned_conversion");
            let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
            zombie_mob.mob_write_nbt(&mut nbt);
            assert_eq!(nbt.get_int("DrownedConversionTime"), Some(300));
        }

        // 3. Husk Zombie conversion tracker
        {
            let husk = from_type(&EntityType::HUSK, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let husk_mob = husk.cast_any().downcast_ref::<crate::entity::mob::zombie::husk::HuskEntity>()
                .expect("Husk downcast");
            assert!(!husk_mob.is_converting(), "Husk must not be converting initially");
            husk_mob.start_drowned_conversion(300);
            assert!(husk_mob.is_converting(), "Husk must be converting after start_drowned_conversion");
            let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
            husk_mob.mob_write_nbt(&mut nbt);
            assert_eq!(nbt.get_int("DrownedConversionTime"), Some(300));
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn creaking_sniffer_armadillo_camel_frog_parity_tests() {
        let (world, _dir) = create_test_world();

        // 1. Creaking parity
        {
            let creaking = from_type(&EntityType::CREAKING, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = creaking.get_mob().expect("Creaking is mob");
            assert!(mob.as_creaking().is_some());
            assert_eq!(mob.get_min_attack_interval(), Some(40), "Creaking attack interval must be 40 ticks");
            assert!(!mob.burns_in_daylight(), "Creaking must not burn in daylight");
            assert!(!mob.can_drown(), "Creaking must not drown");

            let living = creaking.get_living_entity().expect("Creaking is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 1.0);
            assert!((living.get_attribute_base(&Attributes::MOVEMENT_SPEED) - 0.4).abs() < 1e-6);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 32.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0625);

            let creaking_entity = mob.as_creaking().unwrap();
            creaking_entity.set_transient(pumpkin_util::math::position::BlockPos::new(0, 64, 0));
            assert!(creaking_entity.is_heart_bound(), "Creaking must be heart bound after set_transient");

            // Normal damage is absorbed by heart
            assert!(!mob.pre_damage(DamageType::GENERIC, None), "Heart-bound creaking must be invulnerable to generic damage");
            // Damage that bypasses invulnerability penetrates
            assert!(mob.pre_damage(DamageType::OUT_OF_WORLD, None), "Out of world damage must bypass invulnerability");
        }

        // 2. Sniffer parity
        {
            let sniffer = from_type(&EntityType::SNIFFER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = sniffer.get_mob().expect("Sniffer is mob");
            assert!(mob.as_sniffer().is_some());

            let living = sniffer.get_living_entity().expect("Sniffer is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.1);
            assert_eq!(living.health.load(), 14.0);
        }

        // 3. Armadillo parity
        {
            let armadillo = from_type(&EntityType::ARMADILLO, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = armadillo.get_mob().expect("Armadillo is mob");
            let living = armadillo.get_living_entity().expect("Armadillo is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 12.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.14);
            assert_eq!(living.health.load(), 12.0);
            assert_eq!(mob.get_max_head_rotation(), 32.5);

            let armadillo_entity = mob.as_armadillo().expect("Armadillo downcast");
            armadillo_entity.switch_to_state(crate::entity::passive::armadillo::ArmadilloState::Scared);
            assert_eq!(mob.get_max_head_rotation(), 0.0, "Head yaw rotation must be 0 while scared");

            // Damage reduction while rolled up: (dmg - 1.0) / 2.0
            assert_eq!(mob.adjust_damage_amount(7.0, &DamageType::GENERIC), 3.0);
            armadillo_entity.switch_to_state(crate::entity::passive::armadillo::ArmadilloState::Idle);
            assert_eq!(mob.adjust_damage_amount(7.0, &DamageType::GENERIC), 7.0);
        }

        // 4. Camel & Camel Husk parity
        {
            let camel = from_type(&EntityType::CAMEL, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = camel.get_mob().expect("Camel is mob");
            let living = camel.get_living_entity().expect("Camel is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 32.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.09);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.5);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.42);
            assert_eq!(living.health.load(), 32.0);
            assert_eq!(mob.get_max_head_rotation(), 30.0);
            assert!(!mob.remove_when_far_away(1000.0), "Normal camel must not despawn when far away");

            let husk = from_type(&EntityType::CAMEL_HUSK, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let husk_mob = husk.get_mob().expect("CamelHusk is mob");
            let husk_living = husk.get_living_entity().expect("CamelHusk is living");
            assert_eq!(husk_living.get_attribute_base(&Attributes::MAX_HEALTH), 32.0);
            assert_eq!(husk_living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.09);
            assert_eq!(husk_living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.5);
            assert_eq!(husk_living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.42);
            assert_eq!(husk_mob.get_max_head_rotation(), 30.0);
            assert!(husk_mob.remove_when_far_away(1000.0), "CamelHusk must despawn when far away");
        }

        // 5. Frog parity
        {
            let frog = from_type(&EntityType::FROG, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = frog.get_mob().expect("Frog is mob");
            let living = frog.get_living_entity().expect("Frog is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 10.0);
            assert_eq!(mob.get_max_head_rotation(), 5.0);
            assert!(!mob.can_drown(), "Frog must not drown");
            assert!(mob.can_breathe_under_water(), "Frog breathes underwater");

            // Fall damage reduction of 5
            assert_eq!(mob.adjust_damage_amount(8.0, &DamageType::FALL), 3.0);
            assert_eq!(mob.adjust_damage_amount(4.0, &DamageType::FALL), 0.0);
            assert_eq!(mob.adjust_damage_amount(8.0, &DamageType::GENERIC), 8.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_goat_fox_panda_axolotl_dolphin_parity() {
        use crate::entity::passive::panda::PandaGene;
        use std::sync::atomic::Ordering;

        let (world, _dir) = create_test_world();

        // 1. Goat Parity (Gap 77)
        {
            let goat = from_type(&EntityType::GOAT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = goat.get_mob().expect("Goat is mob");
            let living = goat.get_living_entity().expect("Goat is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 10.0);
            assert!(mob.as_goat().is_some());

            // Fall damage reduction of 10
            assert_eq!(mob.adjust_damage_amount(15.0, &DamageType::FALL), 5.0);
            assert_eq!(mob.adjust_damage_amount(10.0, &DamageType::FALL), 0.0);
            assert_eq!(mob.adjust_damage_amount(6.0, &DamageType::FALL), 0.0);
            assert_eq!(mob.adjust_damage_amount(15.0, &DamageType::GENERIC), 15.0);
        }

        // 2. Fox Parity (Gap 78)
        {
            let fox = from_type(&EntityType::FOX, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = fox.get_mob().expect("Fox is mob");
            let living = fox.get_living_entity().expect("Fox is living");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.get_attribute_base(&Attributes::SAFE_FALL_DISTANCE), 5.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 32.0);
            assert_eq!(living.health.load(), 10.0);
            assert!(mob.get_mob_entity().can_pick_up_loot());
            assert!(mob.as_fox().is_some());

            // Sweet berry bush damage cancelled
            assert!(!mob.pre_damage(DamageType::SWEET_BERRY_BUSH, None));
            assert!(mob.pre_damage(DamageType::GENERIC, None));
        }

        // 3. Panda Parity (Gap 79)
        {
            let panda = from_type(&EntityType::PANDA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = panda.get_mob().expect("Panda is mob");
            let living = panda.get_living_entity().expect("Panda is living");
            let panda_entity = mob.as_panda().expect("PandaEntity downcast");

            // Attack damage base
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);

            // Genetics logic
            assert!(PandaGene::Brown.is_recessive());
            assert!(PandaGene::Weak.is_recessive());
            assert!(!PandaGene::Normal.is_recessive());
            assert!(!PandaGene::Lazy.is_recessive());

            assert_eq!(PandaGene::get_variant_from_genes(PandaGene::Weak, PandaGene::Weak), PandaGene::Weak);
            assert_eq!(PandaGene::get_variant_from_genes(PandaGene::Weak, PandaGene::Normal), PandaGene::Normal);
            assert_eq!(PandaGene::get_variant_from_genes(PandaGene::Normal, PandaGene::Weak), PandaGene::Normal);
            assert_eq!(PandaGene::get_variant_from_genes(PandaGene::Lazy, PandaGene::Normal), PandaGene::Lazy);

            // Set Weak gene
            panda_entity.set_main_gene(PandaGene::Weak);
            panda_entity.set_hidden_gene(PandaGene::Weak);
            assert!(panda_entity.is_weak());
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.health.load(), 10.0);

            // Set Lazy gene
            panda_entity.set_main_gene(PandaGene::Lazy);
            panda_entity.set_hidden_gene(PandaGene::Normal);
            assert!(panda_entity.is_lazy());
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.07);
        }

        // 4. Axolotl Parity (Gap 80)
        {
            let axolotl = from_type(&EntityType::AXOLOTL, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = axolotl.get_mob().expect("Axolotl is mob");
            let living = axolotl.get_living_entity().expect("Axolotl is living");
            assert!(mob.as_axolotl().is_some());
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert!(!mob.can_drown(), "Axolotl must not drown");
            assert!(mob.can_breathe_under_water(), "Axolotl breathes underwater");
            assert!(mob.dries_out_on_land(), "Axolotl dries out on land");
            assert!(mob.rehydrates_in_rain(), "Axolotl rehydrates in rain");
            assert_eq!(mob.max_air_supply(), 6000);
            assert!(!axolotl.is_pushed_by_fluids());
        }

        // 5. Dolphin Parity (Gap 81)
        {
            let dolphin = from_type(&EntityType::DOLPHIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = dolphin.get_mob().expect("Dolphin is mob");
            let living = dolphin.get_living_entity().expect("Dolphin is living");
            let dolphin_entity = mob.as_dolphin().expect("Dolphin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.2);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.breath.air_supply.load(Ordering::Relaxed), 4800);
            assert_eq!(mob.max_air_supply(), 4800);
            assert_eq!(mob.increase_air_supply(200), 4800);
            assert_eq!(mob.get_max_head_rotation(), 1.0);
            assert!(!mob.can_breathe_under_water(), "Dolphins cannot breathe underwater (they drown without air)");
            assert_eq!(dolphin_entity.moistness_level.load(Ordering::Relaxed), 2400);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_turtle_polar_bear_rabbit_snow_golem_parity() {
        let (world, _dir) = create_test_world();

        // 1. Turtle Parity (Gap 82)
        {
            let turtle = from_type(&EntityType::TURTLE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = turtle.get_mob().expect("Turtle is mob");
            let living = turtle.get_living_entity().expect("Turtle is living");
            assert!(mob.as_turtle().is_some(), "Turtle downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 30.0);
            assert!(mob.can_breathe_under_water(), "Turtles breathe underwater");
            assert!(!mob.can_drown(), "Turtles never drown");
            assert!(!turtle.is_pushed_by_fluids(), "Turtles not pushed by fluids");
            assert_eq!(mob.get_ambient_sound_interval(), 200);
        }

        // 2. Polar Bear Parity (Gap 83)
        {
            let bear = from_type(&EntityType::POLAR_BEAR, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = bear.get_mob().expect("Polar bear is mob");
            let living = bear.get_living_entity().expect("Polar bear is living");
            assert!(mob.as_polar_bear().is_some(), "Polar bear downcast");
            assert!(mob.as_neutral().is_some(), "Polar bear is neutral");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 30.0);
        }

        // 3. Rabbit & Killer Bunny Parity (Gap 84)
        {
            let rabbit = from_type(&EntityType::RABBIT, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = rabbit.get_mob().expect("Rabbit is mob");
            let living = rabbit.get_living_entity().expect("Rabbit is living");
            let rabbit_entity = mob.as_rabbit().expect("Rabbit downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.health.load(), 3.0);

            // Test Killer Bunny mutation parity
            rabbit_entity.set_variant(crate::entity::passive::rabbit::RabbitVariant::Evil);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 8.0);

            // Return to normal
            rabbit_entity.set_variant(crate::entity::passive::rabbit::RabbitVariant::Brown);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 0.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
        }

        // 4. Snow Golem Parity (Gap 85)
        {
            let golem = from_type(&EntityType::SNOW_GOLEM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = golem.get_mob().expect("Snow golem is mob");
            let living = golem.get_living_entity().expect("Snow golem is living");
            assert!(mob.as_snow_golem().is_some(), "Snow golem downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 4.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.health.load(), 4.0);
            assert!(mob.is_sensitive_to_water(), "Snow golem is sensitive to water");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_copper_golem_happy_ghast_nautilus_parity() {
        let (world, _dir) = create_test_world();

        // 1. Copper Golem Parity (Gap 86)
        {
            let golem = from_type(&EntityType::COPPER_GOLEM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = golem.get_mob().expect("Copper golem is mob");
            let living = golem.get_living_entity().expect("Copper golem is living");
            assert!(mob.as_copper_golem().is_some(), "Copper golem downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 12.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.health.load(), 12.0);
        }

        // 2. Happy Ghast Parity (Gap 87)
        {
            let ghast = from_type(&EntityType::HAPPY_GHAST, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = ghast.get_mob().expect("Happy ghast is mob");
            let living = ghast.get_living_entity().expect("Happy ghast is living");
            assert!(mob.as_happy_ghast().is_some(), "Happy ghast downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::TEMPT_RANGE), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::FLYING_SPEED), 0.05);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.05);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::CAMERA_DISTANCE), 8.0);
            assert_eq!(living.health.load(), 20.0);
            assert!(!mob.can_breathe_under_water(), "Adult happy ghast cannot breathe underwater");
            assert!(mob.can_drown(), "Adult happy ghast can drown");
        }

        // 3. Nautilus Parity (Gap 88)
        {
            let nautilus = from_type(&EntityType::NAUTILUS, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = nautilus.get_mob().expect("Nautilus is mob");
            let living = nautilus.get_living_entity().expect("Nautilus is living");
            assert!(mob.as_nautilus().is_some(), "Nautilus downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 15.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.3);
            assert_eq!(living.health.load(), 15.0);
            assert!(mob.can_breathe_under_water(), "Nautilus breathes underwater");
            assert!(!mob.can_drown(), "Nautilus never drowns");
            assert!(!nautilus.is_pushed_by_fluids(), "Nautilus not pushed by fluids");
            assert_eq!(mob.max_air_supply(), 300);
            assert!(mob.dries_out_on_land());
        }

        // 4. Zombie Nautilus Parity (Gap 89)
        {
            let znautilus = from_type(&EntityType::ZOMBIE_NAUTILUS, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = znautilus.get_mob().expect("Zombie nautilus is mob");
            let living = znautilus.get_living_entity().expect("Zombie nautilus is living");
            assert!(mob.as_zombie_nautilus().is_some(), "Zombie nautilus downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 15.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 1.1);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.3);
            assert_eq!(living.health.load(), 15.0);
            assert!(mob.can_breathe_under_water(), "Zombie nautilus breathes underwater");
            assert!(!mob.can_drown(), "Zombie nautilus never drowns");
            assert!(!znautilus.is_pushed_by_fluids(), "Zombie nautilus not pushed by fluids");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_wolf_llama_golems_phantom_cat_parity() {
        let (world, _dir) = create_test_world();

        // 1. Wolf & Llama Interaction & Targets (Gap 90)
        {
            let wolf = from_type(&EntityType::WOLF, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let wolf_mob = wolf.get_mob().expect("Wolf is mob");
            let wolf_goals = wolf_mob.get_mob_entity().goals_selector.lock().unwrap();
            let wolf_targets = wolf_mob.get_mob_entity().target_selector.lock().unwrap();

            // Check wolf has AvoidEntityGoal for Llama and Trader Llama
            assert!(wolf_goals.get_available_goals().len() >= 10, "Wolf has avoid llama goals");
            // Check wolf has skeleton targets (Skeleton, Wither Skeleton, Stray, Bogged, Parched)
            let target_goals = wolf_targets.get_available_goals();
            let skeleton_targets = target_goals.iter().filter(|g| g.priority == 7).count();
            assert_eq!(skeleton_targets, 5, "Wolf must target 5 skeleton variants: Skeleton, Wither Skeleton, Stray, Bogged, Parched");

            let llama = from_type(&EntityType::LLAMA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let llama_mob = llama.get_mob().expect("Llama is mob");
            let llama_targets = llama_mob.get_mob_entity().target_selector.lock().unwrap();
            assert!(llama_targets.get_available_goals().len() >= 2, "Llama has revenge and wolf target goals");

            let trader_llama = from_type(&EntityType::TRADER_LLAMA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let trader_llama_mob = trader_llama.get_mob().expect("Trader llama is mob");
            let trader_llama_targets = trader_llama_mob.get_mob_entity().target_selector.lock().unwrap();
            assert!(trader_llama_targets.get_available_goals().len() >= 4, "Trader llama targets untamed wolves, zombies, husks, drowned");
        }

        // 2. Iron Golem & Copper Golem Underwater Respiration (Gap 91)
        {
            let iron_golem = from_type(&EntityType::IRON_GOLEM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let iron_mob = iron_golem.get_mob().expect("Iron golem is mob");
            assert!(iron_mob.can_breathe_under_water(), "Iron golem breathes underwater (parity with vanilla decreaseAirSupply override)");
            assert!(!iron_mob.can_drown(), "Iron golem never drowns");

            let copper_golem = from_type(&EntityType::COPPER_GOLEM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let copper_mob = copper_golem.get_mob().expect("Copper golem is mob");
            assert!(copper_mob.can_breathe_under_water(), "Copper golem breathes underwater");
            assert!(!copper_mob.can_drown(), "Copper golem never drowns");
        }

        // 3. Phantom Cat Avoidance (Gap 92)
        {
            let phantom = from_type(&EntityType::PHANTOM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = phantom.get_mob().expect("Phantom is mob");
            let goals = mob.get_mob_entity().goals_selector.lock().unwrap();
            assert!(goals.get_available_goals().len() >= 3, "Phantom has attack strategy, sweep attack, circle goals");
            let targets = mob.get_mob_entity().target_selector.lock().unwrap();
            assert_eq!(targets.get_available_goals().len(), 1, "Phantom has player attack target goal");
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_equine_mobs_parity() {
        let (world, _dir) = create_test_world();

        // 1. Horse Base Attributes Parity (Gap 93)
        {
            let horse = from_type(&EntityType::HORSE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = horse.get_mob().expect("Horse is mob");
            let living = horse.get_living_entity().expect("Horse is living");
            assert!(mob.as_horse().is_some(), "Horse downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 53.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.225);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.7);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::SAFE_FALL_DISTANCE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::FALL_DAMAGE_MULTIPLIER), 0.5);
            assert_eq!(living.health.load(), 53.0);
        }

        // 2. Donkey & Mule Base Attributes Parity
        {
            let donkey = from_type(&EntityType::DONKEY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = donkey.get_mob().expect("Donkey is mob");
            let living = donkey.get_living_entity().expect("Donkey is living");
            assert!(mob.as_donkey().is_some(), "Donkey downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 53.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);
            assert_eq!(living.get_attribute_base(&Attributes::STEP_HEIGHT), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::SAFE_FALL_DISTANCE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::FALL_DAMAGE_MULTIPLIER), 0.5);
            assert_eq!(living.health.load(), 53.0);

            let mule = from_type(&EntityType::MULE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = mule.get_mob().expect("Mule is mob");
            let living = mule.get_living_entity().expect("Mule is living");
            assert!(mob.as_mule().is_some(), "Mule downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 53.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);
            assert_eq!(living.health.load(), 53.0);
        }

        // 3. Skeleton Horse & Zombie Horse Attributes, Undead Breathing Parity
        {
            let skeleton_horse = from_type(&EntityType::SKELETON_HORSE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = skeleton_horse.get_mob().expect("Skeleton horse is mob");
            let living = skeleton_horse.get_living_entity().expect("Skeleton horse is living");
            assert!(mob.as_skeleton_horse().is_some(), "Skeleton horse downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 15.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.2);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.7);
            assert_eq!(living.health.load(), 15.0);
            assert!(mob.can_breathe_under_water(), "Skeleton horse breathes underwater");
            assert!(!mob.can_drown(), "Skeleton horse never drowns");

            let zombie_horse = from_type(&EntityType::ZOMBIE_HORSE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zombie_horse.get_mob().expect("Zombie horse is mob");
            let living = zombie_horse.get_living_entity().expect("Zombie horse is living");
            assert!(mob.as_zombie_horse().is_some(), "Zombie horse downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 25.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.225);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.7);
            assert_eq!(living.health.load(), 25.0);
            assert!(mob.can_breathe_under_water(), "Zombie horse breathes underwater");
            assert!(!mob.can_drown(), "Zombie horse never drowns");
        }

        // 4. Llama & Trader Llama Attributes Parity
        {
            let llama = from_type(&EntityType::LLAMA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = llama.get_mob().expect("Llama is mob");
            let living = llama.get_living_entity().expect("Llama is living");
            assert!(mob.as_llama().is_some(), "Llama downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 53.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);
            assert_eq!(living.health.load(), 53.0);

            let trader_llama = from_type(&EntityType::TRADER_LLAMA, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = trader_llama.get_mob().expect("Trader llama is mob");
            let living = trader_llama.get_living_entity().expect("Trader llama is living");
            assert!(mob.as_trader_llama().is_some(), "Trader llama downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 53.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.get_attribute_base(&Attributes::JUMP_STRENGTH), 0.5);
            assert_eq!(living.health.load(), 53.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_vex_squids_hoglin_zoglin_parity() {
        let (world, _dir) = create_test_world();

        // 1. Vex Attributes, Health & Flight Parity (Gap 94)
        {
            let vex = from_type(&EntityType::VEX, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = vex.get_mob().expect("Vex is mob");
            let living = vex.get_living_entity().expect("Vex is living");
            assert!(mob.as_vex().is_some(), "Vex downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 14.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 4.0);
            assert_eq!(living.health.load(), 14.0);
            assert!(vex.get_entity().no_physics.load(std::sync::atomic::Ordering::Relaxed));
            assert!(vex.get_entity().has_no_gravity());
        }

        // 2. Squid & Glow Squid Water Respiration & Downcasts Parity
        {
            let squid = from_type(&EntityType::SQUID, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = squid.get_mob().expect("Squid is mob");
            let living = squid.get_living_entity().expect("Squid is living");
            assert!(mob.as_squid().is_some(), "Squid downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.health.load(), 10.0);
            assert!(mob.can_breathe_under_water(), "Squid breathes underwater");
            assert!(!mob.can_drown(), "Squid never drowns");
            assert!(!squid.is_pushed_by_fluids(), "Squid fluid push immune");
            assert!(mob.dries_out_on_land());

            let glow_squid = from_type(&EntityType::GLOW_SQUID, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = glow_squid.get_mob().expect("Glow squid is mob");
            let living = glow_squid.get_living_entity().expect("Glow squid is living");
            assert!(mob.as_glow_squid().is_some(), "Glow squid downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.health.load(), 10.0);
            assert!(mob.can_breathe_under_water(), "Glow squid breathes underwater");
            assert!(!mob.can_drown(), "Glow squid never drowns");
            assert!(!glow_squid.is_pushed_by_fluids(), "Glow squid fluid push immune");
            assert!(mob.dries_out_on_land());
        }

        // 3. Hoglin & Zoglin Attributes & Downcasts Parity
        {
            let hoglin = from_type(&EntityType::HOGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = hoglin.get_mob().expect("Hoglin is mob");
            let living = hoglin.get_living_entity().expect("Hoglin is living");
            assert!(mob.as_hoglin().is_some(), "Hoglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_KNOCKBACK), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 40.0);

            let zoglin = from_type(&EntityType::ZOGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zoglin.get_mob().expect("Zoglin is mob");
            let living = zoglin.get_living_entity().expect("Zoglin is living");
            assert!(mob.as_zoglin().is_some(), "Zoglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_KNOCKBACK), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 40.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_strider_cubes_silverfish_endermite_parity() {
        let (world, _temp_dir) = create_test_world();

        // 1. Strider Suffocating Speed & Water Sensitivity Parity (Gap 95)
        {
            let strider_ent = from_type(&EntityType::STRIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = strider_ent.get_mob().expect("Strider is mob");
            let living = strider_ent.get_living_entity().expect("Strider is living");
            let strider = mob.as_strider().expect("Strider downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
            assert_eq!(living.health.load(), 20.0);
            assert!(mob.is_sensitive_to_water(), "Strider is sensitive to water");

            // Out of lava / suffocating reduces speed by 34% (Strider.java:81-83)
            strider.set_suffocating(true);
            assert!(strider.is_suffocating());
            let suffocating_speed = living.get_attribute_base(&Attributes::MOVEMENT_SPEED);
            assert!((suffocating_speed - (0.175 * 0.66)).abs() < 1e-4);

            // Back in lava restores base speed 0.175
            strider.set_suffocating(false);
            assert!(!strider.is_suffocating());
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.175);
        }

        // 2. Slime & Magma Cube Size Attributes Parity (Gap 95)
        {
            let slime_ent = from_type(&EntityType::SLIME, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = slime_ent.get_mob().expect("Slime is mob");
            let living = slime_ent.get_living_entity().expect("Slime is living");
            assert!(mob.as_cube_mob().is_some(), "CubeMob downcast");
            let slime = mob.as_slime().expect("Slime downcast");
            assert!(mob.as_magma_cube().is_none(), "Slime is not magma cube");

            slime.set_size(1, true);
            assert!(slime.is_tiny());
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 1.0);
            assert_eq!(living.health.load(), 1.0);

            slime.set_size(4, true);
            assert!(!slime.is_tiny());
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 4.0);
            assert_eq!(living.health.load(), 16.0);

            let magma_ent = from_type(&EntityType::MAGMA_CUBE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = magma_ent.get_mob().expect("Magma cube is mob");
            let living = magma_ent.get_living_entity().expect("Magma cube is living");
            assert!(mob.as_cube_mob().is_some(), "CubeMob downcast");
            let magma = mob.as_magma_cube().expect("Magma cube downcast");
            assert!(mob.as_slime().is_none(), "Magma cube is not slime");

            magma.set_size(1, true);
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 1.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0); // 1 + 2
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 3.0); // 1 * 3
            assert_eq!(living.health.load(), 1.0);

            magma.set_size(4, true);
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0); // 4 + 2
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 12.0); // 4 * 3
            assert_eq!(living.health.load(), 16.0);
        }

        // 3. Silverfish & Endermite Parity (Gap 96)
        {
            let silverfish_ent = from_type(&EntityType::SILVERFISH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = silverfish_ent.get_mob().expect("Silverfish is mob");
            let living = silverfish_ent.get_living_entity().expect("Silverfish is living");
            assert!(mob.as_silverfish().is_some(), "Silverfish downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 1.0);
            assert_eq!(living.health.load(), 8.0);

            let endermite_ent = from_type(&EntityType::ENDERMITE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = endermite_ent.get_mob().expect("Endermite is mob");
            let living = endermite_ent.get_living_entity().expect("Endermite is living");
            assert!(mob.as_endermite().is_some(), "Endermite downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 8.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 8.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_hostile_mobs_downcasts_and_parity() {
        let (world, _temp_dir) = create_test_world();

        // 1. Creeper
        {
            let ent = from_type(&EntityType::CREEPER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = ent.get_mob().expect("Creeper is mob");
            let living = ent.get_living_entity().expect("Creeper is living");
            assert!(mob.as_creeper().is_some(), "Creeper downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 20.0);
        }

        // 2. Spider & Cave Spider
        {
            let spider_ent = from_type(&EntityType::SPIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = spider_ent.get_mob().expect("Spider is mob");
            let living = spider_ent.get_living_entity().expect("Spider is living");
            assert!(mob.as_spider().is_some(), "Spider downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 16.0);

            let cave_ent = from_type(&EntityType::CAVE_SPIDER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = cave_ent.get_mob().expect("Cave spider is mob");
            let living = cave_ent.get_living_entity().expect("Cave spider is living");
            assert!(mob.as_cave_spider().is_some(), "Cave spider downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 12.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 2.0);
            assert_eq!(living.health.load(), 12.0);
        }

        // 3. Enderman
        {
            let enderman_ent = from_type(&EntityType::ENDERMAN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = enderman_ent.get_mob().expect("Enderman is mob");
            let living = enderman_ent.get_living_entity().expect("Enderman is living");
            assert!(mob.as_enderman().is_some(), "Enderman downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 7.0);
            assert_eq!(living.health.load(), 40.0);
            assert!(mob.is_sensitive_to_water(), "Enderman is water sensitive");
        }

        // 4. Witch
        {
            let witch_ent = from_type(&EntityType::WITCH, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = witch_ent.get_mob().expect("Witch is mob");
            let living = witch_ent.get_living_entity().expect("Witch is living");
            assert!(mob.as_witch().is_some(), "Witch downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 26.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 26.0);
        }

        // 5. Blaze
        {
            let blaze_ent = from_type(&EntityType::BLAZE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = blaze_ent.get_mob().expect("Blaze is mob");
            let living = blaze_ent.get_living_entity().expect("Blaze is living");
            assert!(mob.as_blaze().is_some(), "Blaze downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.health.load(), 20.0);
            assert!(mob.is_sensitive_to_water(), "Blaze is water sensitive");
        }

        // 6. Ghast
        {
            let ghast_ent = from_type(&EntityType::GHAST, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = ghast_ent.get_mob().expect("Ghast is mob");
            let living = ghast_ent.get_living_entity().expect("Ghast is living");
            assert!(mob.as_ghast().is_some(), "Ghast downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 10.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 100.0);
            assert_eq!(living.health.load(), 10.0);
        }

        // 7. Shulker
        {
            let shulker_ent = from_type(&EntityType::SHULKER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = shulker_ent.get_mob().expect("Shulker is mob");
            let living = shulker_ent.get_living_entity().expect("Shulker is living");
            assert!(mob.as_shulker().is_some(), "Shulker downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 30.0);
            assert_eq!(living.health.load(), 30.0);
        }

        // 8. Phantom
        {
            let phantom_ent = from_type(&EntityType::PHANTOM, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = phantom_ent.get_mob().expect("Phantom is mob");
            let living = phantom_ent.get_living_entity().expect("Phantom is living");
            assert!(mob.as_phantom().is_some(), "Phantom downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.health.load(), 20.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_zombie_and_skeleton_variants_parity() {
        let (world, _temp_dir) = create_test_world();

        // 1. Zombie Variants (Zombie, Drowned, Husk, Zombie Villager, Zombified Piglin)
        {
            let zombie_ent = from_type(&EntityType::ZOMBIE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zombie_ent.get_mob().expect("Zombie is mob");
            let living = zombie_ent.get_living_entity().expect("Zombie is living");
            assert!(mob.as_zombie().is_some(), "Zombie downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);
            assert_eq!(living.health.load(), 20.0);

            let drowned_ent = from_type(&EntityType::DROWNED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = drowned_ent.get_mob().expect("Drowned is mob");
            let living = drowned_ent.get_living_entity().expect("Drowned is living");
            assert!(mob.as_drowned().is_some(), "Drowned downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);
            assert!(mob.can_breathe_under_water());

            let husk_ent = from_type(&EntityType::HUSK, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = husk_ent.get_mob().expect("Husk is mob");
            let living = husk_ent.get_living_entity().expect("Husk is living");
            assert!(mob.as_husk().is_some(), "Husk downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);

            let zv_ent = from_type(&EntityType::ZOMBIE_VILLAGER, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zv_ent.get_mob().expect("Zombie villager is mob");
            let living = zv_ent.get_living_entity().expect("Zombie villager is living");
            assert!(mob.as_zombie_villager().is_some(), "Zombie villager downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 3.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);

            let zp_ent = from_type(&EntityType::ZOMBIFIED_PIGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zp_ent.get_mob().expect("Zombified piglin is mob");
            let living = zp_ent.get_living_entity().expect("Zombified piglin is living");
            assert!(mob.as_zombified_piglin().is_some(), "Zombified piglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.23);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 5.0);
            assert_eq!(living.get_attribute_base(&Attributes::ARMOR), 2.0);
        }

        // 2. Skeleton Variants (Skeleton, Wither Skeleton, Stray, Bogged, Parched)
        {
            let skel_ent = from_type(&EntityType::SKELETON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = skel_ent.get_mob().expect("Skeleton is mob");
            let living = skel_ent.get_living_entity().expect("Skeleton is living");
            assert!(mob.as_skeleton().is_some(), "Skeleton downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 20.0);

            let wither_ent = from_type(&EntityType::WITHER_SKELETON, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = wither_ent.get_mob().expect("Wither skeleton is mob");
            let living = wither_ent.get_living_entity().expect("Wither skeleton is living");
            assert!(mob.as_wither_skeleton().is_some(), "Wither skeleton downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 4.0);
            assert_eq!(living.health.load(), 20.0);

            let stray_ent = from_type(&EntityType::STRAY, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = stray_ent.get_mob().expect("Stray is mob");
            let living = stray_ent.get_living_entity().expect("Stray is living");
            assert!(mob.as_stray().is_some(), "Stray downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 20.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 20.0);

            let bogged_ent = from_type(&EntityType::BOGGED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = bogged_ent.get_mob().expect("Bogged is mob");
            let living = bogged_ent.get_living_entity().expect("Bogged is living");
            assert!(mob.as_bogged().is_some(), "Bogged downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 16.0);

            let parched_ent = from_type(&EntityType::PARCHED, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = parched_ent.get_mob().expect("Parched is mob");
            let living = parched_ent.get_living_entity().expect("Parched is living");
            assert!(mob.as_parched().is_some(), "Parched downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.25);
            assert_eq!(living.health.load(), 16.0);
        }

        world.level.shutdown().await;
    }

    #[tokio::test]
    async fn test_piglin_family_parity() {
        let (world, _temp_dir) = create_test_world();

        // 1. Piglin
        {
            let piglin_ent = from_type(&EntityType::PIGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = piglin_ent.get_mob().expect("Piglin is mob");
            let living = piglin_ent.get_living_entity().expect("Piglin is living");
            assert!(mob.as_piglin().is_some(), "Piglin downcast");
            assert!(mob.as_abstract_piglin().is_some(), "Piglin abstract piglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 16.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.35);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 5.0);
            assert_eq!(living.health.load(), 16.0);
        }

        // 2. Piglin Brute
        {
            let brute_ent = from_type(&EntityType::PIGLIN_BRUTE, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = brute_ent.get_mob().expect("Piglin Brute is mob");
            let living = brute_ent.get_living_entity().expect("Piglin Brute is living");
            assert!(mob.as_piglin_brute().is_some(), "Piglin Brute downcast");
            assert!(mob.as_abstract_piglin().is_some(), "Piglin Brute abstract piglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 50.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.35);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 7.0);
            assert_eq!(living.get_attribute_base(&Attributes::FOLLOW_RANGE), 12.0);
            assert_eq!(living.health.load(), 50.0);
        }

        // 3. Hoglin
        {
            let hoglin_ent = from_type(&EntityType::HOGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = hoglin_ent.get_mob().expect("Hoglin is mob");
            let living = hoglin_ent.get_living_entity().expect("Hoglin is living");
            assert!(mob.as_hoglin().is_some(), "Hoglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_KNOCKBACK), 1.0);
            assert_eq!(living.health.load(), 40.0);
        }

        // 4. Zoglin
        {
            let zoglin_ent = from_type(&EntityType::ZOGLIN, Vector3::new(0.0, 64.0, 0.0), &world, Uuid::new_v4());
            let mob = zoglin_ent.get_mob().expect("Zoglin is mob");
            let living = zoglin_ent.get_living_entity().expect("Zoglin is living");
            assert!(mob.as_zoglin().is_some(), "Zoglin downcast");
            assert_eq!(living.get_attribute_base(&Attributes::MAX_HEALTH), 40.0);
            assert_eq!(living.get_attribute_base(&Attributes::MOVEMENT_SPEED), 0.3);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_DAMAGE), 6.0);
            assert_eq!(living.get_attribute_base(&Attributes::KNOCKBACK_RESISTANCE), 0.6);
            assert_eq!(living.get_attribute_base(&Attributes::ATTACK_KNOCKBACK), 1.0);
            assert_eq!(living.health.load(), 40.0);
        }

        world.level.shutdown().await;
    }
}


