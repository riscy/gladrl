// DON'T EDIT: automatically generated by make_skills_registry.sh
use actor::Actor;
use world::World;
use plan::Plan;
use skills::*;

pub fn should_use_skill(actor: &mut Actor, wld: &World, p: &Plan) -> bool {
    actor.selected_skill = 0;
    choose_skill!(should_sprint if can_sprint => actor, wld, p);
    choose_skill!(should_charge if can_charge => actor, wld, p);
    choose_skill!(should_cloak if can_cloak => actor, wld, p);
    choose_skill!(should_shoot if can_shoot => actor, wld, p);
    choose_skill!(should_barrage if can_barrage => actor, wld, p);
    choose_skill!(should_boomerang if can_boomerang => actor, wld, p);
    choose_skill!(should_warp_space if can_warp_space => actor, wld, p);
    choose_skill!(should_blast if can_blast => actor, wld, p);
    choose_skill!(should_teleport if can_teleport => actor, wld, p);
    choose_skill!(should_heal if can_heal => actor, wld, p);
    choose_skill!(should_lie if can_lie => actor, wld, p);
    choose_skill!(should_summon_faerie if can_summon_faerie => actor, wld, p);
    choose_skill!(should_grow_tree if can_grow_tree => actor, wld, p);
    choose_skill!(should_expand if can_expand => actor, wld, p);
    choose_skill!(should_multiply if can_multiply => actor, wld, p);
    choose_skill!(should_spawn_elf if can_spawn_elf => actor, wld, p);
    choose_skill!(should_spawn_dead if can_spawn_dead => actor, wld, p);
    choose_skill!(should_spawn_mage if can_spawn_mage => actor, wld, p);
    choose_skill!(should_pick if can_pick => actor, wld, p);
    false
}

pub fn use_skill(actor: &mut Actor, wld: &mut World, p: &Plan, spawn: &mut Vec<Actor>) {
    use_skill!(sprint if can_sprint => actor, wld, p, spawn);
    use_skill!(charge if can_charge => actor, wld, p, spawn);
    use_skill!(cloak if can_cloak => actor, wld, p, spawn);
    use_skill!(shoot if can_shoot => actor, wld, p, spawn);
    use_skill!(barrage if can_barrage => actor, wld, p, spawn);
    use_skill!(boomerang if can_boomerang => actor, wld, p, spawn);
    use_skill!(warp_space if can_warp_space => actor, wld, p, spawn);
    use_skill!(blast if can_blast => actor, wld, p, spawn);
    use_skill!(teleport if can_teleport => actor, wld, p, spawn);
    use_skill!(heal if can_heal => actor, wld, p, spawn);
    use_skill!(lie if can_lie => actor, wld, p, spawn);
    use_skill!(summon_faerie if can_summon_faerie => actor, wld, p, spawn);
    use_skill!(grow_tree if can_grow_tree => actor, wld, p, spawn);
    use_skill!(expand if can_expand => actor, wld, p, spawn);
    use_skill!(multiply if can_multiply => actor, wld, p, spawn);
    use_skill!(spawn_elf if can_spawn_elf => actor, wld, p, spawn);
    use_skill!(spawn_dead if can_spawn_dead => actor, wld, p, spawn);
    use_skill!(spawn_mage if can_spawn_mage => actor, wld, p, spawn);
    use_skill!(pick if can_pick => actor, wld, p, spawn);
}
