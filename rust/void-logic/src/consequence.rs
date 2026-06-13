//! Contact → consequence: the pure decision layer between the kinetic
//! world's contact events and gameplay effects. The shell classifies
//! bodies (it owns the id registry) and applies the effects; every
//! rule about who gets hurt, and when, lives here, pinned by tests.

use crate::audio_catalog::COLLISION_SFX_MIN_SPEED;
use crate::kinetic_world::{BodyId, ContactEvent, ContactWith};
use crate::newtypes::Damage;
use crate::ram_damage;

/// What a body is, gameplay-wise. The shell's registry answers this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Player,
    Enemy,
    Prop,
    Bolt,
}

/// The gameplay effect of one contact. Momentum exchange itself is the
/// world's business and needs no consequence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Consequence {
    None,
    /// The player's hull hit level geometry hard enough to hear.
    HullImpact { position: [f32; 3] },
    /// A bolt detonates on whatever it touched; `struck_player` says
    /// whether its payload applies (the shell holds the payload).
    BoltImpact { bolt: BodyId, struck_player: bool },
    /// Player↔enemy ram: one symmetric rule, damage precomputed.
    Ram {
        enemy: BodyId,
        enemy_damage: Damage,
        player_damage: Damage,
    },
}

/// Decide the consequence of a contact. `kind_of` classifies ids.
pub fn consequence_of(
    contact: &ContactEvent,
    kind_of: impl Fn(BodyId) -> BodyKind,
) -> Consequence {
    let first = (contact.body, kind_of(contact.body));
    let second = match contact.with {
        ContactWith::Body(other) => Some((other, kind_of(other))),
        ContactWith::Static => None,
    };
    let participants = [Some(first), second];

    // Bolts detonate on anything they touch; their payload applies
    // only when the other party is the player.
    if let Some((bolt, _)) = participants
        .into_iter()
        .flatten()
        .find(|(_, kind)| *kind == BodyKind::Bolt)
    {
        let struck_player = participants
            .into_iter()
            .flatten()
            .any(|(id, kind)| id != bolt && kind == BodyKind::Player);
        return Consequence::BoltImpact { bolt, struck_player };
    }

    // Static contact: only the player's hull is audible, and only
    // above the SFX threshold.
    let Some(second) = second else {
        if first.1 == BodyKind::Player && contact.impact_speed > COLLISION_SFX_MIN_SPEED {
            return Consequence::HullImpact {
                position: contact.position,
            };
        }
        return Consequence::None;
    };

    // Player↔enemy: the one symmetric ram rule, both directions.
    let enemy = match (first.1, second.1) {
        (BodyKind::Player, BodyKind::Enemy) => Some(second.0),
        (BodyKind::Enemy, BodyKind::Player) => Some(first.0),
        _ => None,
    };
    if let Some(enemy) = enemy {
        let damage = ram_damage::ram_damage(contact.impact_speed);
        if damage.as_f32() > 0.0 {
            return Consequence::Ram {
                enemy,
                enemy_damage: damage,
                player_damage: Damage::new(damage.as_f32() * ram_damage::PLAYER_RAM_FRACTION),
            };
        }
    }
    Consequence::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(index: usize) -> BodyId {
        BodyId::from_index(index)
    }

    /// ids: 0 = player, 1 = enemy, 2 = prop, 3 = bolt, 4 = enemy.
    fn kinds(body: BodyId) -> BodyKind {
        match body.index() {
            0 => BodyKind::Player,
            1 | 4 => BodyKind::Enemy,
            3 => BodyKind::Bolt,
            _ => BodyKind::Prop,
        }
    }

    fn contact(body: usize, with: ContactWith, impact_speed: f32) -> ContactEvent {
        ContactEvent {
            body: id(body),
            with,
            normal: [1.0, 0.0, 0.0],
            impact_speed,
            position: [1.0, 2.0, 3.0],
        }
    }

    #[test]
    fn bolts_detonate_on_anything_and_only_hurt_the_player() {
        for (other, hurts) in [
            (ContactWith::Body(id(0)), true),
            (ContactWith::Body(id(1)), false),
            (ContactWith::Body(id(2)), false),
            (ContactWith::Static, false),
        ] {
            let result = consequence_of(&contact(3, other, 5.0), kinds);
            assert_eq!(
                result,
                Consequence::BoltImpact {
                    bolt: id(3),
                    struck_player: hurts
                },
                "bolt vs {other:?}"
            );
        }
        // And reported from the other side of the pair:
        let result = consequence_of(&contact(0, ContactWith::Body(id(3)), 5.0), kinds);
        assert_eq!(
            result,
            Consequence::BoltImpact {
                bolt: id(3),
                struck_player: true
            },
            "pair order must not matter"
        );
    }

    #[test]
    fn rams_are_order_symmetric() {
        let a = consequence_of(&contact(0, ContactWith::Body(id(1)), 12.0), kinds);
        let b = consequence_of(&contact(1, ContactWith::Body(id(0)), 12.0), kinds);
        assert_eq!(a, b, "who reports the pair must not matter");
        assert!(matches!(a, Consequence::Ram { enemy, .. } if enemy == id(1)));
    }

    #[test]
    fn ram_damage_follows_the_model() {
        let result = consequence_of(&contact(0, ContactWith::Body(id(4)), 12.0), kinds);
        let expected = ram_damage::ram_damage(12.0);
        let Consequence::Ram {
            enemy,
            enemy_damage,
            player_damage,
        } = result
        else {
            panic!("expected a ram, got {result:?}");
        };
        assert_eq!(enemy, id(4));
        assert_eq!(enemy_damage, expected);
        assert_eq!(
            player_damage,
            Damage::new(expected.as_f32() * ram_damage::PLAYER_RAM_FRACTION)
        );
    }

    #[test]
    fn gentle_rams_are_silent() {
        let result = consequence_of(&contact(0, ContactWith::Body(id(1)), 2.0), kinds);
        assert_eq!(result, Consequence::None, "below MIN_RAM_SPEED: a nudge");
    }

    #[test]
    fn hull_scrapes_are_heard_only_above_the_sfx_threshold() {
        let loud = consequence_of(
            &contact(0, ContactWith::Static, COLLISION_SFX_MIN_SPEED + 1.0),
            kinds,
        );
        assert_eq!(
            loud,
            Consequence::HullImpact {
                position: [1.0, 2.0, 3.0]
            }
        );
        let soft = consequence_of(
            &contact(0, ContactWith::Static, COLLISION_SFX_MIN_SPEED - 1.0),
            kinds,
        );
        assert_eq!(soft, Consequence::None);
    }

    #[test]
    fn contacts_without_the_player_are_silent() {
        for (a, b) in [
            (2, ContactWith::Body(id(1))), // prop-enemy
            (1, ContactWith::Body(id(4))), // enemy-enemy
            (1, ContactWith::Static),      // enemy-wall
            (2, ContactWith::Static),      // prop-wall
            (0, ContactWith::Body(id(2))), // player-prop: momentum only
        ] {
            assert_eq!(
                consequence_of(&contact(a, b, 50.0), kinds),
                Consequence::None,
                "{a} vs {b:?}"
            );
        }
    }
}
