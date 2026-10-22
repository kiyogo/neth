use std::{collections::BTreeMap, fs, path::Path};

use heck::ToShoutySnakeCase;
use proc_macro2::{Ident, Span, TokenStream};
use pumpkin_util::{
    identifier::Identifier,
    loot_table::{
        LootBonusFormula, LootCondition, LootEntityPredicate, LootEntityProperty,
        LootEntityPropertyValue, LootEntityTarget, LootFunction,
    },
};
use quote::{format_ident, quote};
use serde::Deserialize;
use syn::LitStr;

/// `rolls` can be a bare float or an object with `type/min/max`.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum RollsStruct {
    Constant(f32),
    Provider {
        #[allow(dead_code)]
        #[serde(rename = "type")]
        provider_type: String,
        #[allow(dead_code)]
        #[serde(default)]
        min: f32,
        #[serde(default)]
        max: f32,
    },
}

impl RollsStruct {
    fn min(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { min, .. } => min.round() as i32,
        }
    }
    fn max(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { max, .. } => max.round() as i32,
        }
    }
}

/// A `set_count` count provider (uniform or constant).
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum CountStruct {
    Constant(f32),
    Provider {
        #[serde(rename = "type")]
        #[allow(dead_code)]
        provider_type: String,
        #[serde(default)]
        min: f32,
        #[serde(default)]
        max: f32,
    },
}

impl CountStruct {
    fn min(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { min, .. } => min.round() as i32,
        }
    }
    fn max(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { max, .. } => max.round() as i32,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct PredicateStruct {
    #[serde(default)]
    items: Option<serde_json::Value>,
    #[serde(default)]
    predicates: Option<serde_json::Value>,
    #[serde(flatten)]
    entity_properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum EnchantedChanceStruct {
    Constant(f32),
    Linear {
        #[serde(rename = "type")]
        chance_type: String,
        base: f32,
        #[serde(default)]
        per_level_above_first: f32,
    },
}

/// A condition is either an inline definition or a reference to a `predicate` file.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum ConditionValue {
    Reference(String),
    Inline(Box<ConditionStruct>),
}

#[derive(Deserialize, Clone, Debug)]
struct ConditionStruct {
    #[serde(rename = "type", default)]
    condition: String,
    #[allow(dead_code)]
    #[serde(default)]
    enchantment: Option<String>,
    #[serde(default)]
    chance: Option<f32>,
    #[serde(default)]
    unenchanted_chance: Option<f32>,
    #[serde(default)]
    enchanted_chance: Option<EnchantedChanceStruct>,
    #[serde(default)]
    chances: Option<Vec<f32>>,
    #[serde(default)]
    predicate: Option<PredicateStruct>,
    #[serde(default)]
    entity: Option<String>,
    #[serde(default)]
    term: Option<ConditionValue>,
    #[serde(default)]
    terms: Option<Vec<ConditionValue>>,
    #[serde(default)]
    properties: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    state: Option<serde_json::Map<String, serde_json::Value>>,
}

fn resolve_condition(value: &ConditionValue) -> LootCondition {
    match value {
        ConditionValue::Inline(cond) => parse_condition(cond),
        ConditionValue::Reference(name) => {
            let relative = name.strip_prefix("minecraft:").unwrap_or(name);
            let path = Path::new("../../assets/datapack/data/minecraft/predicate")
                .join(format!("{relative}.json"));
            fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<ConditionStruct>(&content).ok())
                .map_or(LootCondition::None, |cond| parse_condition(&cond))
        }
    }
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

/// Combines conditions that must all hold, dropping unconditional parts.
fn all_of(parts: Vec<LootCondition>) -> LootCondition {
    let mut parts: Vec<LootCondition> = parts
        .into_iter()
        .filter(|c| *c != LootCondition::None)
        .collect();
    parts.dedup();
    match parts.len() {
        0 => LootCondition::None,
        1 => parts[0],
        _ => LootCondition::AllOf(Box::leak(parts.into_boxed_slice())),
    }
}

fn invert(cond: LootCondition) -> LootCondition {
    match cond {
        LootCondition::SilkTouch => LootCondition::NoSilkTouch,
        LootCondition::NoSilkTouch => LootCondition::SilkTouch,
        LootCondition::SilkTouchOrShears => LootCondition::NoSilkTouchOrShears,
        LootCondition::NoSilkTouchOrShears => LootCondition::SilkTouchOrShears,
        LootCondition::Inverted(inner) => *inner,
        other => LootCondition::Inverted(Box::leak(Box::new(other))),
    }
}

/// Whether a condition gives the same answer every time it is checked, so it can be
/// negated without re-rolling a random chance.
fn is_deterministic(cond: LootCondition) -> bool {
    match cond {
        LootCondition::None
        | LootCondition::SilkTouch
        | LootCondition::NoSilkTouch
        | LootCondition::Shears
        | LootCondition::SilkTouchOrShears
        | LootCondition::NoSilkTouchOrShears
        | LootCondition::KilledByPlayer
        | LootCondition::BlockStateProperty { .. }
        | LootCondition::EntityProperties { .. }
        | LootCondition::EntityOnFire
        | LootCondition::DirectAttackerWeaponEnchantment(_) => true,
        LootCondition::Inverted(inner) => is_deterministic(*inner),
        LootCondition::AllOf(list) | LootCondition::AnyOf(list) => {
            list.iter().copied().all(is_deterministic)
        }
        LootCondition::SurvivesExplosion
        | LootCondition::RandomChance { .. }
        | LootCondition::RandomChanceWithEnchantedBonus { .. }
        | LootCondition::TableBonus { .. } => false,
    }
}

/// Translate one vanilla loot condition into its generated [`LootCondition`].
///
/// Conditions the runtime cannot evaluate are mapped to [`LootCondition::None`], which keeps
/// the owning entry unconditional rather than dropping it.
fn parse_condition(cond: &ConditionStruct) -> LootCondition {
    match cond.condition.as_str() {
        "minecraft:block_state_property" | "minecraft:match_block" => {
            let Some(properties) = cond.properties.as_ref().or(cond.state.as_ref()) else {
                return LootCondition::None;
            };
            let mut pairs = Vec::new();
            for (name, value) in properties {
                let Some(value) = value.as_str() else {
                    return LootCondition::None;
                };
                pairs.push((leak_str(name), leak_str(value)));
            }
            if pairs.is_empty() {
                LootCondition::None
            } else {
                LootCondition::BlockStateProperty {
                    properties: Box::leak(pairs.into_boxed_slice()),
                }
            }
        }
        "minecraft:survives_explosion" => LootCondition::SurvivesExplosion,
        "minecraft:killed_by_player" => LootCondition::KilledByPlayer,
        "minecraft:random_chance" => {
            let chance = cond
                .chance
                .or_else(|| cond.chances.as_ref().and_then(|c| c.first().copied()))
                .unwrap_or(0.0);
            LootCondition::RandomChance { chance }
        }
        "minecraft:random_chance_with_enchanted_bonus" => {
            let unenchanted_chance = cond.unenchanted_chance.unwrap_or(0.0);
            let (enchanted_chance_base, enchanted_chance_per_level_above_first) =
                match &cond.enchanted_chance {
                    Some(EnchantedChanceStruct::Linear {
                        base,
                        per_level_above_first,
                        ..
                    }) => (*base, *per_level_above_first),
                    Some(EnchantedChanceStruct::Constant(c)) => (*c, 0.0),
                    None => (unenchanted_chance, 0.0),
                };
            LootCondition::RandomChanceWithEnchantedBonus {
                unenchanted_chance,
                enchanted_chance_base,
                enchanted_chance_per_level_above_first,
            }
        }
        "minecraft:entity_properties" => {
            let Some(target_name) = cond.entity.as_deref() else {
                return LootCondition::None;
            };
            let target = match target_name {
                "this" => LootEntityTarget::This,
                "attacker" => LootEntityTarget::Attacker,
                "direct_attacker" => LootEntityTarget::DirectAttacker,
                _ => return LootCondition::None,
            };

            let Some(predicate) = &cond.predicate else {
                return LootCondition::None;
            };

            let mut properties = Vec::new();
            let mut skipped = Vec::new();
            for (key, value) in &predicate.entity_properties {
                flatten_entity_properties(key, value, &mut properties, &mut skipped);
            }

            if properties.is_empty() {
                if !skipped.is_empty() {
                    eprintln!(
                        "loot_table: no value of the `{target_name}` entity predicate can be \
                         expressed ({}), leaving the entry unconditional",
                        skipped.join(", ")
                    );
                }
                return LootCondition::None;
            }
            if !skipped.is_empty() {
                eprintln!(
                    "loot_table: the `{target_name}` entity predicate is only partly expressed, \
                     ignoring {}",
                    skipped.join(", ")
                );
            }

            LootCondition::EntityProperties {
                target,
                predicate: LootEntityPredicate {
                    properties: Box::leak(properties.into_boxed_slice()),
                },
            }
        }
        "minecraft:table_bonus" => {
            let chances = cond.chances.clone().unwrap_or_default();
            if chances.is_empty() {
                LootCondition::None
            } else {
                LootCondition::TableBonus {
                    chances: Box::leak(chances.into_boxed_slice()),
                }
            }
        }
        "minecraft:all_of" => {
            if let Some(terms) = &cond.terms {
                combine_conditions(terms)
            } else {
                LootCondition::None
            }
        }
        "minecraft:match_tool" => {
            if let Some(pred) = &cond.predicate {
                if let Some(items_val) = &pred.items {
                    let is_shears = match items_val {
                        serde_json::Value::String(s) => s.contains("shears"),
                        serde_json::Value::Array(arr) => arr
                            .iter()
                            .any(|v| v.as_str().is_some_and(|s| s.contains("shears"))),
                        _ => false,
                    };
                    if is_shears {
                        return LootCondition::Shears;
                    }
                }
                if let Some(pred_val) = &pred.predicates {
                    let s = pred_val.to_string();
                    if s.contains("silk_touch") {
                        return LootCondition::SilkTouch;
                    }
                }
            }
            LootCondition::None
        }
        "minecraft:any_of" => {
            if let Some(terms) = &cond.terms {
                let has_silk = terms
                    .iter()
                    .any(|t| resolve_condition(t) == LootCondition::SilkTouch);
                let has_shears = terms
                    .iter()
                    .any(|t| resolve_condition(t) == LootCondition::Shears);
                if has_silk && has_shears {
                    return LootCondition::SilkTouchOrShears;
                } else if has_silk {
                    return LootCondition::SilkTouch;
                } else if has_shears {
                    return LootCondition::Shears;
                }
                let parsed: Vec<LootCondition> = terms.iter().map(resolve_condition).collect();
                if !parsed.contains(&LootCondition::None) {
                    return LootCondition::AnyOf(Box::leak(parsed.into_boxed_slice()));
                }
            }
            LootCondition::None
        }
        "minecraft:entity_properties" => parse_entity_properties(cond),
        "minecraft:inverted" => {
            if let Some(term) = &cond.term {
                match resolve_condition(term) {
                    LootCondition::SilkTouch => LootCondition::NoSilkTouch,
                    LootCondition::Shears => LootCondition::NoSilkTouchOrShears,
                    LootCondition::SilkTouchOrShears => LootCondition::NoSilkTouchOrShears,
                    LootCondition::None => LootCondition::None,
                    other => LootCondition::Inverted(Box::leak(Box::new(other))),
                }
            } else {
                LootCondition::None
            }
        }
        _ => LootCondition::None,
    }
}

fn parse_entity_properties(cond: &ConditionStruct) -> LootCondition {
    let Some(predicate) = &cond.predicate else {
        return LootCondition::None;
    };
    match cond.entity.as_deref() {
        Some("this")
            if predicate
                .entity_properties
                .get("minecraft:flags")
                .and_then(|flags| flags.get("is_on_fire"))
                .and_then(serde_json::Value::as_bool)
                == Some(true) =>
        {
            return LootCondition::EntityOnFire;
        }
        Some("direct_attacker") => {
            if let Some(tag) = predicate
                .entity_properties
                .get("minecraft:equipment")
                .and_then(|equipment| {
                    equipment
                        .pointer("/mainhand/predicates/minecraft:enchantments/0/enchantments")?
                        .as_str()?
                        .strip_prefix('#')
                })
            {
                return LootCondition::DirectAttackerWeaponEnchantment(Box::leak(tag.into()));
            }
        }
        _ => {}
    }

    let Some(target_name) = cond.entity.as_deref() else {
        return LootCondition::None;
    };
    let target = match target_name {
        "this" => LootEntityTarget::This,
        "attacker" => LootEntityTarget::Attacker,
        "direct_attacker" => LootEntityTarget::DirectAttacker,
        _ => return LootCondition::None,
    };

    let mut properties = Vec::new();
    let mut skipped = Vec::new();
    for (key, value) in &predicate.entity_properties {
        flatten_entity_properties(key, value, &mut properties, &mut skipped);
    }

    if properties.is_empty() {
        if !skipped.is_empty() {
            eprintln!(
                "loot_table: no value of the `{target_name}` entity predicate can be \
                 expressed ({}), leaving the entry unconditional",
                skipped.join(", ")
            );
        }
        return LootCondition::None;
    }
    if !skipped.is_empty() {
        eprintln!(
            "loot_table: the `{target_name}` entity predicate is only partly expressed, \
             ignoring {}",
            skipped.join(", ")
        );
    }

    LootCondition::EntityProperties {
        target,
        predicate: LootEntityPredicate {
            properties: Box::leak(properties.into_boxed_slice()),
        },
    }
}

/// Flatten a nested entity predicate into `path/to/key` and primitive value pairs.
///
/// `path` is the prefix accumulated so far; nested objects recurse with their key appended.
/// Values the runtime has no representation for (arrays, floats, `null`) are collected in
/// `skipped` instead of being matched on. The condition then only checks what is left, which
/// stays closer to vanilla than dropping the whole condition would; a predicate made up of
/// nothing but those ends up empty and its condition becomes [`LootCondition::None`], the same
/// permissive fallback every condition the runtime cannot evaluate gets. Both cases are
/// reported by [`parse_condition`] so an unrepresentable predicate is visible when the data is
/// regenerated.
fn flatten_entity_properties(
    path: &str,
    value: &serde_json::Value,
    properties: &mut Vec<LootEntityProperty>,
    skipped: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                flatten_entity_properties(&format!("{path}/{key}"), value, properties, skipped);
            }
        }
        serde_json::Value::Bool(value) => properties.push(LootEntityProperty {
            key: Box::leak(path.to_owned().into_boxed_str()),
            value: LootEntityPropertyValue::Bool(*value),
        }),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                properties.push(LootEntityProperty {
                    key: Box::leak(path.to_owned().into_boxed_str()),
                    value: LootEntityPropertyValue::Integer(value),
                });
            } else {
                skipped.push(format!("{path} (non-integer number)"));
            }
        }
        serde_json::Value::String(value) => properties.push(LootEntityProperty {
            key: Box::leak(path.to_owned().into_boxed_str()),
            value: LootEntityPropertyValue::String(Box::leak(value.clone().into_boxed_str())),
        }),
        serde_json::Value::Array(_) => skipped.push(format!("{path} (array)")),
        serde_json::Value::Null => skipped.push(format!("{path} (null)")),
    }
}

/// Resolve an optional condition, treating a missing one as [`LootCondition::None`].
fn condition_of(value: Option<&ConditionValue>) -> LootCondition {
    value.map_or(LootCondition::None, resolve_condition)
}

/// Combine every condition on a pool or entry into a single [`LootCondition`].
///
/// Unrepresentable conditions drop out, a single condition is returned as-is, and two or more
/// are wrapped in [`LootCondition::AllOf`].
fn combine_conditions(conditions: &[ConditionValue]) -> LootCondition {
    let mut parsed_list: Vec<LootCondition> = Vec::new();
    for c in conditions {
        let parsed = resolve_condition(c);
        if parsed != LootCondition::None {
            parsed_list.push(parsed);
        }
    }
    match parsed_list.len() {
        0 => LootCondition::None,
        1 => parsed_list[0],
        _ => LootCondition::AllOf(Box::leak(parsed_list.into_boxed_slice())),
    }
}

#[derive(Deserialize, Clone, Debug)]
struct BonusParameterStruct {
    #[serde(rename = "bonusMultiplier", default)]
    bonus_multiplier: Option<i32>,
    #[serde(default)]
    extra: Option<i32>,
    #[serde(default)]
    probability: Option<f32>,
}

/// Item modifiers hold either a single entry or a list of them.
pub fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    // `Many` is tried first because `serde_json::Value` would also match `One`.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        Many(Vec<T>),
        One(T),
    }

    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::Many(values) => values,
        OneOrMany::One(value) => vec![value],
    })
}

#[derive(Deserialize, Clone, Debug)]
struct CopyComponentsStruct {
    source: String,
    #[serde(default, deserialize_with = "deserialize_component_filter")]
    include: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_component_filter")]
    exclude: Option<Vec<String>>,
    #[serde(default)]
    condition: Option<ConditionValue>,
    #[serde(default, deserialize_with = "one_or_many")]
    conditions: Vec<ConditionValue>,
}

/// Decode an explicitly present filter and normalize registry names; null and malformed names are errors.
fn deserialize_component_filter<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|name| {
            Identifier::parse(&name)
                .map(|identifier| identifier.to_string())
                .map_err(serde::de::Error::custom)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

#[derive(Deserialize, Clone, Debug)]
struct EntryFunctionJson {
    #[serde(rename = "type", alias = "function")]
    function: String,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    parameters: Option<BonusParameterStruct>,
    count: Option<CountStruct>,
    #[serde(default)]
    condition: Option<ConditionValue>,
    #[serde(default, deserialize_with = "one_or_many")]
    conditions: Vec<ConditionValue>,
    #[serde(flatten)]
    remaining: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(try_from = "EntryFunctionJson")]
struct EntryFunctionStruct {
    function: String,
    formula: Option<String>,
    parameters: Option<BonusParameterStruct>,
    count: Option<CountStruct>,
    condition: Option<ConditionValue>,
    conditions: Vec<ConditionValue>,
    copy_components: Option<CopyComponentsStruct>,
}

impl TryFrom<EntryFunctionJson> for EntryFunctionStruct {
    type Error = String;

    /// Validate supported component functions while retaining the existing count and bonus fields.
    fn try_from(raw: EntryFunctionJson) -> Result<Self, Self::Error> {
        let copy_components = if raw.function == "minecraft:copy_components" {
            let mut copy: CopyComponentsStruct =
                serde_json::from_value(serde_json::Value::Object(raw.remaining))
                    .map_err(|error| format!("invalid copy_components function: {error}"))?;
            if copy.source != "block_entity" {
                return Err(format!(
                    "unsupported copy_components source: {}",
                    copy.source
                ));
            }
            if copy.condition.is_none() {
                copy.condition = raw.condition.clone();
            }
            if copy.conditions.is_empty() {
                copy.conditions = raw.conditions.clone();
            }
            Some(copy)
        } else {
            None
        };
        Ok(Self {
            function: raw.function,
            formula: raw.formula,
            parameters: raw.parameters,
            count: raw.count,
            condition: raw.condition,
            conditions: raw.conditions,
            copy_components,
        })
    }
}

/// Emit an optional registry-name filter without conflating omission and an empty list.
fn component_filter_to_tokens(filter: Option<&[String]>) -> TokenStream {
    match filter {
        None => quote! { None },
        Some(names) => {
            let names = names
                .iter()
                .map(|name| LitStr::new(name, Span::call_site()));
            quote! { Some(&[#(#names),*]) }
        }
    }
}

/// Emit the component source, filters, and conditions for one ordered function.
fn function_to_tokens(function: &CopyComponentsStruct) -> TokenStream {
    let source = LitStr::new(&function.source, Span::call_site());
    let include = component_filter_to_tokens(function.include.as_deref());
    let exclude = component_filter_to_tokens(function.exclude.as_deref());
    let cond = combine_pair(
        condition_of(function.condition.as_ref()),
        combine_conditions(&function.conditions),
    );
    let condition = condition_to_tokens(cond);
    quote! {
        LootFunction::CopyComponents {
            source: #source,
            include: #include,
            exclude: #exclude,
            condition: #condition,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum LootTableValue {
    Reference(String),
    Inline(ChestLootTableJson),
}

#[derive(Deserialize, Clone, Debug)]
struct PoolEntryStruct {
    #[serde(rename = "type")]
    entry_type: String,
    name: Option<String>,
    /// Tag entries name their tag here.
    #[serde(default)]
    items: Option<String>,
    #[serde(default)]
    value: Option<LootTableValue>,
    #[serde(default = "default_weight")]
    weight: i32,
    #[serde(alias = "functions", rename = "modifier", default, deserialize_with = "one_or_many")]
    functions: Vec<EntryFunctionStruct>,
    #[serde(default)]
    condition: Option<ConditionValue>,
    #[serde(default, deserialize_with = "one_or_many")]
    conditions: Vec<ConditionValue>,
    #[serde(default)]
    children: Vec<PoolEntryStruct>,
}

fn default_weight() -> i32 {
    1
}

#[derive(Deserialize, Clone, Debug)]
struct PoolStruct {
    #[serde(default)]
    entries: Vec<PoolEntryStruct>,
    #[serde(default = "default_rolls")]
    rolls: RollsStruct,
    #[serde(default)]
    condition: Option<ConditionValue>,
    #[serde(default, deserialize_with = "one_or_many")]
    conditions: Vec<ConditionValue>,
    #[serde(alias = "modifier", default, deserialize_with = "one_or_many")]
    functions: Vec<EntryFunctionStruct>,
}

fn default_rolls() -> RollsStruct {
    RollsStruct::Constant(1.0)
}

#[derive(Deserialize, Clone, Debug)]
struct ChestLootTableJson {
    #[serde(default)]
    pools: Vec<PoolStruct>,
}

fn path_to_key(relative: &str) -> String {
    format!("minecraft:{relative}")
}

fn path_to_ident(relative: &str) -> String {
    relative.replace('/', "_").to_shouty_snake_case()
}

struct ParsedEntry {
    item: String,
    weight: i32,
    min_count: i32,
    max_count: i32,
    condition: LootCondition,
    bonus_formula: Option<LootBonusFormula>,
    furnace_smelt: Option<LootCondition>,
    functions: Vec<CopyComponentsStruct>,
}

/// Require both conditions, dropping only a [`LootCondition::None`], which adds nothing.
///
/// Two structurally equal conditions are kept as a pair on purpose: the runtime evaluates every
/// element of a [`LootCondition::AllOf`] on its own, and a stateful one such as
/// [`LootCondition::RandomChance`] rolls once per evaluation, exactly as vanilla rolls the pool
/// condition and the entry condition separately. Collapsing them would turn a `p * p` chance
/// into `p`.
fn combine_pair(first: LootCondition, second: LootCondition) -> LootCondition {
    match (first, second) {
        (LootCondition::None, cond) | (cond, LootCondition::None) => cond,
        (first, second) => LootCondition::AllOf(Box::leak(vec![first, second].into_boxed_slice())),
    }
}

/// Emits one entry per possible `set_count` outcome. A `set_count` with a condition (e.g.
/// `type=double` on slabs) only applies when it holds, and later ones override earlier ones.
fn push_count_variants(
    item: &str,
    weight: i32,
    condition: LootCondition,
    bonus_formula: Option<LootBonusFormula>,
    furnace_smelt: Option<LootCondition>,
    functions: &[&EntryFunctionStruct],
    copy_components: &[CopyComponentsStruct],
    out: &mut Vec<ParsedEntry>,
) {
    let set_counts: Vec<(LootCondition, (i32, i32))> = functions
        .iter()
        .filter(|f| f.function == "minecraft:set_count")
        .filter_map(|f| {
            f.count.as_ref().map(|c| {
                let single = condition_of(f.condition.as_ref());
                let multi = combine_conditions(&f.conditions);
                let cond = combine_pair(single, multi);
                (cond, (c.min(), c.max()))
            })
        })
        .collect();

    let mut push = |condition: LootCondition, (min_count, max_count): (i32, i32)| {
        out.push(ParsedEntry {
            item: item.to_owned(),
            weight,
            min_count,
            max_count,
            condition,
            bonus_formula,
            furnace_smelt,
            functions: copy_components.to_vec(),
        });
    };

    let all_parsed = functions
        .iter()
        .filter(|f| f.function == "minecraft:set_count")
        .all(|f| {
            let single_ok = f
                .condition
                .as_ref()
                .map_or(true, |c| resolve_condition(c) != LootCondition::None);
            let multi_ok = f
                .conditions
                .iter()
                .all(|c| resolve_condition(c) != LootCondition::None);
            single_ok && multi_ok
        });

    if !all_parsed || !set_counts.iter().all(|(c, _)| is_deterministic(*c)) {
        push(
            condition,
            set_counts.first().map_or((1, 1), |(_, count)| *count),
        );
        return;
    }

    let mut later: Vec<LootCondition> = Vec::new();
    for (set_condition, count) in set_counts.iter().rev() {
        let mut parts = vec![condition, *set_condition];
        parts.extend(later.iter().copied().map(invert));
        push(all_of(parts), *count);
        if *set_condition == LootCondition::None {
            return;
        }
        later.push(*set_condition);
    }
    let mut parts = vec![condition];
    parts.extend(later.iter().copied().map(invert));
    push(all_of(parts), (1, 1));
}

/// Flatten one pool entry into the concrete item entries it can produce.
fn extract_entries(
    entry: &PoolEntryStruct,
    inherited_condition: LootCondition,
    out: &mut Vec<ParsedEntry>,
    empty_weight: &mut i32,
) {
    extract_entries_with_depth(entry, inherited_condition, &[], None, out, empty_weight, 0);
}

/// Recursive worker behind [`extract_entries`], bounded to five levels of nesting.
///
/// `inherited_condition` carries the conditions of every enclosing pool and entry; it is
/// combined with the entry's own conditions so a child can never drop loot while one of its
/// parents' conditions is false.
fn extract_entries_with_depth(
    entry: &PoolEntryStruct,
    inherited_condition: LootCondition,
    pool_functions: &[EntryFunctionStruct],
    inherited_smelt: Option<LootCondition>,
    out: &mut Vec<ParsedEntry>,
    empty_weight: &mut i32,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let entry_own_cond = combine_pair(
        condition_of(entry.condition.as_ref()),
        combine_conditions(&entry.conditions),
    );
    let entry_cond = combine_pair(inherited_condition, entry_own_cond);

    let furnace_smelt = entry
        .functions
        .iter()
        .chain(pool_functions)
        .find(|f| f.function == "minecraft:furnace_smelt")
        .map(|f| {
            combine_pair(
                condition_of(f.condition.as_ref()),
                combine_conditions(&f.conditions),
            )
        })
        .or(inherited_smelt);

    match entry.entry_type.as_str() {
        "minecraft:empty" => {
            *empty_weight += entry.weight;
        }
        "minecraft:item" => {
            if let Some(name) = &entry.name {
                let functions: Vec<&EntryFunctionStruct> =
                    entry.functions.iter().chain(pool_functions).collect();

                let bonus_formula = functions.iter().find_map(|f| {
                    if f.function == "minecraft:apply_bonus" {
                        match f.formula.as_deref() {
                            Some("minecraft:ore_drops") => Some(LootBonusFormula::OreDrops),
                            Some("minecraft:uniform_bonus_count") => {
                                let mult = f
                                    .parameters
                                    .as_ref()
                                    .and_then(|p| p.bonus_multiplier)
                                    .unwrap_or(1);
                                Some(LootBonusFormula::UniformBonusCount(mult))
                            }
                            Some("minecraft:binomial_with_bonus_count") => {
                                let extra =
                                    f.parameters.as_ref().and_then(|p| p.extra).unwrap_or(0);
                                let prob = f
                                    .parameters
                                    .as_ref()
                                    .and_then(|p| p.probability)
                                    .unwrap_or(0.0);
                                Some(LootBonusFormula::BinomialWithBonusCount {
                                    extra,
                                    probability: prob,
                                })
                            }
                            _ => None,
                        }
                    } else if f.function == "minecraft:enchanted_count_increase" {
                        let mult = f.count.as_ref().map_or(1, |c| c.max());
                        Some(LootBonusFormula::UniformBonusCount(mult))
                    } else {
                        None
                    }
                });

                let copy_components: Vec<CopyComponentsStruct> = functions
                    .iter()
                    .filter_map(|f| f.copy_components.clone())
                    .collect();

                push_count_variants(
                    name,
                    entry.weight,
                    entry_cond,
                    bonus_formula,
                    furnace_smelt,
                    &functions,
                    &copy_components,
                    out,
                );
            }
        }
        "minecraft:tag" => {
            let tag_name_opt = entry
                .items
                .as_deref()
                .or(entry.name.as_deref())
                .or_else(|| match &entry.value {
                    Some(LootTableValue::Reference(r)) => Some(r.as_str()),
                    _ => None,
                });
            if let Some(tag_name) = tag_name_opt {
                let tag_name = tag_name.strip_prefix('#').unwrap_or(tag_name);
                let tag_rel = tag_name.strip_prefix("minecraft:").unwrap_or(tag_name);
                let tag_path = Path::new("../../assets/datapack/data/minecraft/tags/item")
                    .join(format!("{tag_rel}.json"));
                if let Ok(content) = fs::read_to_string(&tag_path) {
                    #[derive(Deserialize)]
                    struct TagJson {
                        values: Vec<String>,
                    }
                    if let Ok(tag_data) = serde_json::from_str::<TagJson>(&content) {
                        let copy_components: Vec<CopyComponentsStruct> = entry
                            .functions
                            .iter()
                            .filter_map(|f| f.copy_components.clone())
                            .collect();
                        for item_name in tag_data.values {
                            out.push(ParsedEntry {
                                item: item_name,
                                weight: entry.weight,
                                min_count: 1,
                                max_count: 1,
                                condition: entry_cond,
                                bonus_formula: None,
                                furnace_smelt,
                                functions: copy_components.clone(),
                            });
                        }
                    }
                }
            }
        }
        "minecraft:loot_table" => match &entry.value {
            Some(LootTableValue::Reference(table_name)) => {
                let table_rel = table_name.strip_prefix("minecraft:").unwrap_or(table_name);
                let table_path = Path::new("../../assets/datapack/data/minecraft/loot_table")
                    .join(format!("{table_rel}.json"));
                if let Ok(content) = fs::read_to_string(&table_path) {
                    if let Ok(nested_table) = serde_json::from_str::<ChestLootTableJson>(&content) {
                        for pool in &nested_table.pools {
                            let mut pool_cond = entry_cond;
                            let parsed = combine_pair(
                                condition_of(pool.condition.as_ref()),
                                combine_conditions(&pool.conditions),
                            );
                            if parsed != LootCondition::None {
                                pool_cond = parsed;
                            }
                            for child_entry in &pool.entries {
                                extract_entries_with_depth(
                                    child_entry,
                                    pool_cond,
                                    &pool.functions,
                                    furnace_smelt,
                                    out,
                                    empty_weight,
                                    depth + 1,
                                );
                            }
                        }
                    }
                }
            }
            Some(LootTableValue::Inline(nested_table)) => {
                for pool in &nested_table.pools {
                    let mut pool_cond = entry_cond;
                    let parsed = combine_pair(
                        condition_of(pool.condition.as_ref()),
                        combine_conditions(&pool.conditions),
                    );
                    if parsed != LootCondition::None {
                        pool_cond = parsed;
                    }
                    for child_entry in &pool.entries {
                        extract_entries_with_depth(
                            child_entry,
                            pool_cond,
                            &pool.functions,
                            furnace_smelt,
                            out,
                            empty_weight,
                            depth + 1,
                        );
                    }
                }
            }
            None => {
                if let Some(name) = &entry.name {
                    let table_rel = name.strip_prefix("minecraft:").unwrap_or(name);
                    let table_path = Path::new("../../assets/datapack/data/minecraft/loot_table")
                        .join(format!("{table_rel}.json"));
                    if let Ok(content) = fs::read_to_string(&table_path) {
                        if let Ok(nested_table) =
                            serde_json::from_str::<ChestLootTableJson>(&content)
                        {
                            for pool in &nested_table.pools {
                                let mut pool_cond = entry_cond;
                                let parsed = combine_pair(
                                    condition_of(pool.condition.as_ref()),
                                    combine_conditions(&pool.conditions),
                                );
                                if parsed != LootCondition::None {
                                    pool_cond = parsed;
                                }
                                for child_entry in &pool.entries {
                                    extract_entries_with_depth(
                                        child_entry,
                                        pool_cond,
                                        &pool.functions,
                                        furnace_smelt,
                                        out,
                                        empty_weight,
                                        depth + 1,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        },
        "minecraft:alternatives" => {
            // Vanilla uses the first child whose conditions hold, so a later child only applies
            // when every earlier one fails. Random conditions can't be negated without
            // re-rolling, so only deterministic ones are excluded.
            let mut earlier: Vec<LootCondition> = Vec::new();
            for child in &entry.children {
                let child_cond = combine_pair(
                    condition_of(child.condition.as_ref()),
                    combine_conditions(&child.conditions),
                );
                let mut parts = vec![entry_cond];
                parts.extend(earlier.iter().copied().map(invert));
                extract_entries_with_depth(
                    child,
                    all_of(parts),
                    pool_functions,
                    furnace_smelt,
                    out,
                    empty_weight,
                    depth + 1,
                );
                if child.conditions.is_empty()
                    && child.condition.is_none()
                    && matches!(
                        child.entry_type.as_str(),
                        "minecraft:item" | "minecraft:tag" | "minecraft:empty"
                    )
                {
                    // An unconditional leaf always matches, so later children never run.
                    break;
                }
                // Only exclude conditions that are fully understood: an unparsed one would
                // become "never" and hide every later child.
                let fully_parsed = child
                    .condition
                    .as_ref()
                    .map_or(true, |c| resolve_condition(c) != LootCondition::None)
                    && child
                        .conditions
                        .iter()
                        .all(|c| resolve_condition(c) != LootCondition::None);
                if child_cond != LootCondition::None && fully_parsed && is_deterministic(child_cond)
                {
                    earlier.push(child_cond);
                }
            }
        }
        "minecraft:sequence" | "minecraft:group" => {
            for child in &entry.children {
                extract_entries_with_depth(
                    child,
                    entry_cond,
                    pool_functions,
                    furnace_smelt,
                    out,
                    empty_weight,
                    depth + 1,
                );
            }
        }
        _ => {}
    }
}

/// Emit the `const`-compatible token stream that rebuilds `cond` in the generated data.
fn condition_to_tokens(cond: LootCondition) -> TokenStream {
    match cond {
        LootCondition::None => quote! { LootCondition::None },
        LootCondition::SilkTouch => quote! { LootCondition::SilkTouch },
        LootCondition::NoSilkTouch => quote! { LootCondition::NoSilkTouch },
        LootCondition::Shears => quote! { LootCondition::Shears },
        LootCondition::SilkTouchOrShears => quote! { LootCondition::SilkTouchOrShears },
        LootCondition::NoSilkTouchOrShears => quote! { LootCondition::NoSilkTouchOrShears },
        LootCondition::SurvivesExplosion => quote! { LootCondition::SurvivesExplosion },
        LootCondition::KilledByPlayer => quote! { LootCondition::KilledByPlayer },
        LootCondition::RandomChance { chance } => {
            quote! { LootCondition::RandomChance { chance: #chance } }
        }
        LootCondition::RandomChanceWithEnchantedBonus {
            unenchanted_chance,
            enchanted_chance_base,
            enchanted_chance_per_level_above_first,
        } => {
            quote! {
                LootCondition::RandomChanceWithEnchantedBonus {
                    unenchanted_chance: #unenchanted_chance,
                    enchanted_chance_base: #enchanted_chance_base,
                    enchanted_chance_per_level_above_first: #enchanted_chance_per_level_above_first,
                }
            }
        }
        LootCondition::EntityProperties { target, predicate } => {
            let target_tokens = match target {
                LootEntityTarget::This => quote! { LootEntityTarget::This },
                LootEntityTarget::Attacker => quote! { LootEntityTarget::Attacker },
                LootEntityTarget::DirectAttacker => quote! { LootEntityTarget::DirectAttacker },
            };
            let property_tokens = predicate.properties.iter().map(|property| {
                let key = LitStr::new(property.key, Span::call_site());
                let value = match property.value {
                    LootEntityPropertyValue::Bool(value) => {
                        quote! { LootEntityPropertyValue::Bool(#value) }
                    }
                    LootEntityPropertyValue::Integer(value) => {
                        quote! { LootEntityPropertyValue::Integer(#value) }
                    }
                    LootEntityPropertyValue::String(value) => {
                        let value = LitStr::new(value, Span::call_site());
                        quote! { LootEntityPropertyValue::String(#value) }
                    }
                };
                quote! { LootEntityProperty { key: #key, value: #value } }
            });
            quote! {
                LootCondition::EntityProperties {
                    target: #target_tokens,
                    predicate: LootEntityPredicate {
                        properties: &[#(#property_tokens),*],
                    },
                }
            }
        }
        LootCondition::TableBonus { chances } => {
            let values = chances.iter();
            quote! { LootCondition::TableBonus { chances: &[#(#values),*] } }
        }
        LootCondition::BlockStateProperty { properties } => {
            let pairs = properties
                .iter()
                .map(|(name, value)| quote! { (#name, #value) });
            quote! { LootCondition::BlockStateProperty { properties: &[#(#pairs),*] } }
        }
        LootCondition::Inverted(inner) => {
            let inner = condition_to_tokens(*inner);
            quote! { LootCondition::Inverted(&#inner) }
        }
        LootCondition::AllOf(list) => {
            let tokens: Vec<TokenStream> = list.iter().copied().map(condition_to_tokens).collect();
            quote! { LootCondition::AllOf(&[#(#tokens),*]) }
        }
        LootCondition::AnyOf(list) => {
            let tokens: Vec<TokenStream> = list.iter().copied().map(condition_to_tokens).collect();
            quote! { LootCondition::AnyOf(&[#(#tokens),*]) }
        }
        LootCondition::EntityOnFire => quote! { LootCondition::EntityOnFire },
        LootCondition::DirectAttackerWeaponEnchantment(tag) => {
            quote! { LootCondition::DirectAttackerWeaponEnchantment(#tag) }
        }
    }
}

fn bonus_to_tokens(bonus: Option<LootBonusFormula>) -> TokenStream {
    match bonus {
        None => quote! { None },
        Some(LootBonusFormula::OreDrops) => {
            quote! { Some(LootBonusFormula::OreDrops) }
        }
        Some(LootBonusFormula::UniformBonusCount(mult)) => {
            quote! { Some(LootBonusFormula::UniformBonusCount(#mult)) }
        }
        Some(LootBonusFormula::BinomialWithBonusCount { extra, probability }) => {
            quote! { Some(LootBonusFormula::BinomialWithBonusCount { extra: #extra, probability: #probability }) }
        }
    }
}

/// Emit static entry arrays and pool literals for one table.
/// Returns the list of `LootPool` literals (one per pool).
fn emit_table(
    prefix: &str,
    table: &ChestLootTableJson,
    tokens: &mut TokenStream,
) -> Vec<TokenStream> {
    let mut pool_literals = Vec::new();

    for (pool_idx, pool) in table.pools.iter().enumerate() {
        let min_rolls = pool.rolls.min();
        let max_rolls = pool.rolls.max();

        let pool_cond = combine_pair(
            condition_of(pool.condition.as_ref()),
            combine_conditions(&pool.conditions),
        );

        let mut parsed_entries = Vec::new();
        let mut empty_weight: i32 = 0;

        for entry in &pool.entries {
            extract_entries_with_depth(
                entry,
                LootCondition::None,
                &pool.functions,
                None,
                &mut parsed_entries,
                &mut empty_weight,
                0,
            );
        }

        let entry_literals: Vec<TokenStream> = parsed_entries
            .iter()
            .map(|e| {
                let name_lit = LitStr::new(&e.item, Span::call_site());
                let weight = e.weight;
                let min_count = e.min_count;
                let max_count = e.max_count;
                let cond_tokens = condition_to_tokens(e.condition);
                let bonus_tokens = bonus_to_tokens(e.bonus_formula);
                let smelt_tokens = e.furnace_smelt.map_or_else(
                    || quote! { None },
                    |cond| {
                        let cond = condition_to_tokens(cond);
                        quote! { Some(#cond) }
                    },
                );

                let functions = e.functions.iter().map(function_to_tokens);

                quote! {
                    LootEntry {
                        item: #name_lit,
                        weight: #weight,
                        min_count: #min_count,
                        max_count: #max_count,
                        condition: #cond_tokens,
                        bonus_formula: #bonus_tokens,
                        furnace_smelt: #smelt_tokens,
                        functions: &[#(#functions),*],
                    }
                }
            })
            .collect();

        // Emit the entries static array.
        let entries_ident = format_ident!("{}_POOL{}_ENTRIES", prefix, pool_idx);
        tokens.extend(quote! {
            static #entries_ident: &[LootEntry] = &[#(#entry_literals),*];
        });

        let pool_cond_tokens = condition_to_tokens(pool_cond);

        pool_literals.push(quote! {
            LootPool {
                entries: #entries_ident,
                min_rolls: #min_rolls,
                max_rolls: #max_rolls,
                empty_weight: #empty_weight,
                condition: #pool_cond_tokens,
            }
        });
    }

    pool_literals
}

/// Recursively collect all `*.json` files under `dir`, returning a vec of
/// `(relative_stem_path, parsed_table)`.
fn collect_json_files(base: &Path, dir: &Path) -> Vec<(String, ChestLootTableJson)> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => panic!("failed to read directory {}: {e}", dir.display()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            result.extend(collect_json_files(base, &path));
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => panic!("failed to read {}: {e}", path.display()),
            };

            let table: ChestLootTableJson = match serde_json::from_str(&content) {
                Ok(t) => t,
                Err(e) => panic!("failed to parse {}: {e}", path.display()),
            };

            result.push((relative, table));
        }
    }

    result
}

/// Read every loot JSON from `../../assets/datapack/data/minecraft/loot_table/` (recursively)
/// and emit a `pumpkin-data/src/generated/chest_loot.rs` with static constants
/// and a `get_chest_loot_table(key) -> Option<&'static ChestLootTable>` function.
pub fn build() -> TokenStream {
    let base = Path::new("../../assets/datapack/data/minecraft/loot_table");

    // Collect all JSON files recursively, sorted for deterministic output.
    let mut files: Vec<(String, ChestLootTableJson)> = collect_json_files(base, base);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut all_tokens = TokenStream::new();

    // Emit one set of statics per file
    let mut lookup_rows: Vec<(String, Ident)> = Vec::with_capacity(files.len() * 2);

    for (relative_path, table) in &files {
        let prefix = path_to_ident(relative_path);
        let key = path_to_key(relative_path);
        let table_ident = format_ident!("{}", prefix);

        let pool_tokens = emit_table(&prefix, table, &mut all_tokens);

        let pools_ident = format_ident!("{}_POOLS", prefix);
        all_tokens.extend(quote! {
            static #pools_ident: &[LootPool] = &[#(#pool_tokens),*];
            pub static #table_ident: LootTable = LootTable { pools: #pools_ident };
        });

        lookup_rows.push((key, table_ident.clone()));
        lookup_rows.push((relative_path.clone(), table_ident));
    }
    lookup_rows.sort_by(|a, b| a.0.cmp(&b.0));
    let lookup_len = lookup_rows.len();
    let lookup_rows = lookup_rows
        .iter()
        .map(|(key, ident)| quote! { (#key, &#ident) });

    // Emit get_loot_table and get_chest_loot_table
    all_tokens.extend(quote! {
        static LOOT_TABLES_BY_KEY: [(&str, &LootTable); #lookup_len] = [#(#lookup_rows),*];

        #[must_use]
        pub fn get_loot_table(key: &str) -> Option<&'static LootTable> {
            LOOT_TABLES_BY_KEY
                .binary_search_by_key(&key, |&(k, _)| k)
                .ok()
                .map(|i| LOOT_TABLES_BY_KEY[i].1)
        }

        #[must_use]
        pub fn get_chest_loot_table(key: &str) -> Option<&'static LootTable> {
            get_loot_table(key)
        }
    });

    quote! {
        pub use pumpkin_util::loot_table::*;
        #all_tokens
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_util::loot_table::{
        LootEntityPredicate, LootEntityProperty, LootEntityPropertyValue, LootEntityTarget,
    };

    use super::{
        ConditionStruct, LootCondition, PoolEntryStruct, extract_entries, parse_condition,
    };

    /// The condition the `entity_properties` child in these fixtures parses into.
    const SHEEP_IS_WHITE: LootCondition = LootCondition::EntityProperties {
        target: LootEntityTarget::This,
        predicate: LootEntityPredicate {
            properties: &[LootEntityProperty {
                key: "minecraft:components/minecraft:sheep/color",
                value: LootEntityPropertyValue::String("white"),
            }],
        },
    };

    /// The implicit "no silk touch" gate a tool-gated sibling creates must not replace the
    /// inherited condition either: a later entity-properties child still has to satisfy its
    /// parent.
    #[test]
    fn alternatives_child_after_a_tool_gate_keeps_inherited_condition() {
        let entry: PoolEntryStruct = serde_json::from_str(
            r#"{
                "type": "minecraft:alternatives",
                "condition": { "type": "minecraft:killed_by_player" },
                "children": [
                    {
                        "type": "minecraft:item",
                        "name": "minecraft:white_wool",
                        "condition": {
                            "type": "minecraft:match_tool",
                            "predicate": {
                                "predicates": {
                                    "minecraft:enchantments": [
                                        { "enchantments": "minecraft:silk_touch" }
                                    ]
                                }
                            }
                        }
                    },
                    {
                        "type": "minecraft:item",
                        "name": "minecraft:white_carpet",
                        "condition": {
                            "type": "minecraft:entity_properties",
                            "entity": "this",
                            "predicate": {
                                "minecraft:components": { "minecraft:sheep/color": "white" }
                            }
                        }
                    }
                ]
            }"#,
        )
        .expect("alternatives entry should deserialize");

        let mut entries = Vec::new();
        let mut empty_weight = 0;
        extract_entries(&entry, LootCondition::None, &mut entries, &mut empty_weight);

        assert_eq!(entries.len(), 2);

        // The silk-touch child keeps both its own gate and the parent condition.
        assert_eq!(entries[0].item, "minecraft:white_wool");
        assert_eq!(
            entries[0].condition,
            LootCondition::AllOf(&[LootCondition::KilledByPlayer, LootCondition::SilkTouch])
        );

        // The child after it gets the implicit "no silk touch" gate on top of both.
        assert_eq!(entries[1].item, "minecraft:white_carpet");
        assert_eq!(
            entries[1].condition,
            LootCondition::AllOf(&[
                LootCondition::AllOf(&[LootCondition::KilledByPlayer, LootCondition::NoSilkTouch,]),
                SHEEP_IS_WHITE,
            ])
        );
    }

    /// Children of `minecraft:alternatives` must keep the condition inherited from the
    /// alternatives entry itself. Dropping it would let an entity-property child drop loot
    /// while its parent condition is false.
    #[test]
    fn alternatives_child_keeps_inherited_condition() {
        let entry: PoolEntryStruct = serde_json::from_str(
            r#"{
                "type": "minecraft:alternatives",
                "condition": { "type": "minecraft:killed_by_player" },
                "children": [
                    {
                        "type": "minecraft:item",
                        "name": "minecraft:white_wool",
                        "condition": {
                            "type": "minecraft:entity_properties",
                            "entity": "this",
                            "predicate": {
                                "minecraft:components": { "minecraft:sheep/color": "white" }
                            }
                        }
                    }
                ]
            }"#,
        )
        .expect("alternatives entry should deserialize");

        let mut entries = Vec::new();
        let mut empty_weight = 0;
        extract_entries(&entry, LootCondition::None, &mut entries, &mut empty_weight);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].item, "minecraft:white_wool");
        assert_eq!(
            entries[0].condition,
            LootCondition::AllOf(&[LootCondition::KilledByPlayer, SHEEP_IS_WHITE])
        );
    }

    /// A predicate the runtime cannot express in full still constrains the entry with the
    /// values it does understand; dropping those too would let the entry roll whenever the
    /// unsupported half is all there is to check.
    #[test]
    fn entity_properties_keep_the_values_that_can_be_expressed() {
        let cond: ConditionStruct = serde_json::from_str(
            r#"{
                "type": "minecraft:entity_properties",
                "entity": "this",
                "predicate": {
                    "minecraft:components": { "minecraft:sheep/color": "white" },
                    "minecraft:equipment": {
                        "mainhand": {
                            "predicates": {
                                "minecraft:enchantments": [
                                    { "enchantments": "minecraft:silk_touch" }
                                ]
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("condition should deserialize");

        assert_eq!(parse_condition(&cond), SHEEP_IS_WHITE);
    }

    /// With nothing left to check the condition falls back to [`LootCondition::None`], the
    /// same permissive fallback every unrepresentable condition gets: the entry keeps dropping
    /// as it did before the predicate was understood at all, rather than never dropping.
    #[test]
    fn entity_properties_without_expressible_values_stay_unconditional() {
        let cond: ConditionStruct = serde_json::from_str(
            r##"{
                "type": "minecraft:entity_properties",
                "entity": "direct_attacker",
                "predicate": {
                    "minecraft:equipment": {
                        "mainhand": {
                            "predicates": {
                                "minecraft:enchantments": [
                                    { "enchantments": "#minecraft:smelts_loot" }
                                ]
                            }
                        }
                    }
                }
            }"##,
        )
        .expect("condition should deserialize");

        assert_eq!(parse_condition(&cond), LootCondition::None);
    }

    /// A pool chance and an entry chance that happen to be equal must both survive: the runtime
    /// rolls every element of an [`LootCondition::AllOf`] on its own, the way vanilla rolls the
    /// pool condition and the entry condition separately, so collapsing them would turn a
    /// `p * p` chance into `p`.
    #[test]
    fn equal_stateful_conditions_are_both_kept() {
        let entry: PoolEntryStruct = serde_json::from_str(
            r#"{
                "type": "minecraft:item",
                "name": "minecraft:feather",
                "condition": { "type": "minecraft:random_chance", "chance": 0.5 }
            }"#,
        )
        .expect("entry should deserialize");

        let mut entries = Vec::new();
        let mut empty_weight = 0;
        extract_entries(
            &entry,
            LootCondition::RandomChance { chance: 0.5 },
            &mut entries,
            &mut empty_weight,
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].condition,
            LootCondition::AllOf(&[
                LootCondition::RandomChance { chance: 0.5 },
                LootCondition::RandomChance { chance: 0.5 },
            ])
        );
    }
}
