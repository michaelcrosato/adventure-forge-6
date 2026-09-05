// Literal staffing claims. Action indexes and world times are separate quantities.
#[derive(Clone, Copy, Debug, Serialize)]
struct StaffingHistoryExpectation {
    steps: &'static [StaffingStepExpectation],
    surge_resolution: Option<u64>,
    pending_surge: bool,
    forbidden_memories: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy, Debug, Serialize)]
struct StaffingStepExpectation {
    action_index: usize,
    definition: &'static str,
    before: u64,
    after: u64,
    events: &'static [StaffingEventExpectation],
}

#[derive(Clone, Copy, Debug, Serialize)]
struct StaffingEventExpectation {
    turn: u64,
    kind: StaffingEventKind,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StaffingEventKind {
    PlayerMoved {
        from: &'static str,
        to: &'static str,
    },
    NpcMoved {
        npc: &'static str,
        from: &'static str,
        to: &'static str,
    },
    NpcTransfer {
        npc: &'static str,
        item: &'static str,
        count: u32,
    },
    Resource {
        resource: &'static str,
        amount: i64,
    },
    Time {
        ticks: u64,
    },
    Surge {
        applied: bool,
    },
}

impl StaffingEventExpectation {
    fn event(self) -> forge_kernel::Event {
        use forge_kernel::EventKind;
        let kind = match self.kind {
            StaffingEventKind::PlayerMoved { from, to } => EventKind::Moved {
                from: from.into(),
                to: to.into(),
            },
            StaffingEventKind::NpcMoved { npc, from, to } => EventKind::NpcMoved {
                npc: npc.into(),
                from: from.into(),
                to: to.into(),
            },
            StaffingEventKind::NpcTransfer { npc, item, count } => {
                EventKind::NpcItemTransferredToCharacter {
                    npc: npc.into(),
                    item: item.into(),
                    count,
                }
            }
            StaffingEventKind::Resource { resource, amount } => EventKind::ResourceAdjusted {
                resource: resource.into(),
                amount,
            },
            StaffingEventKind::Time { ticks } => EventKind::TimeAdvanced { ticks },
            StaffingEventKind::Surge { applied } => EventKind::ScheduledEventResolved {
                event_id: "lowsail.next_surge".into(),
                event_kind: "deadline".into(),
                applied,
            },
        };
        forge_kernel::Event {
            turn: self.turn,
            kind,
        }
    }
}

fn staffing_mechanical_event(event: &forge_kernel::Event) -> bool {
    use forge_kernel::EventKind;
    matches!(
        event.kind,
        EventKind::Moved { .. }
            | EventKind::NpcMoved { .. }
            | EventKind::NpcItemTransferredToCharacter { .. }
            | EventKind::ResourceAdjusted { .. }
            | EventKind::TimeAdvanced { .. }
            | EventKind::ScheduledEventResolved { .. }
    )
}

fn validate_staffing_spec(
    history: &StaffingHistoryExpectation,
    spec: &ScenarioSpec,
) -> Result<(), VerifyError> {
    let mut prior = 0;
    for expected in history.steps {
        if expected.action_index <= prior
            || expected.action_index > spec.steps.len()
            || spec.steps[expected.action_index - 1].definition_id != expected.definition
            || expected.before >= expected.after
            || spec
                .expectations
                .final_world_time
                .is_none_or(|end| expected.after > end)
            || expected
                .events
                .iter()
                .any(|event| event.turn < expected.before || event.turn > expected.after)
        {
            return Err(VerifyError::new(
                "staffing history needs exact ordered recipe checkpoints",
            ));
        }
        prior = expected.action_index;
    }
    if history.steps.is_empty()
        || history.pending_surge == history.surge_resolution.is_some()
        || history.surge_resolution.is_some_and(|turn| {
            turn < 16
                || spec
                    .expectations
                    .final_world_time
                    .is_none_or(|end| turn > end)
        })
        || (history.pending_surge
            && spec
                .expectations
                .final_world_time
                .is_none_or(|end| end >= 16))
    {
        return Err(VerifyError::new(
            "staffing history needs an exact absolute deadline disposition",
        ));
    }
    Ok(())
}

fn validate_staffing_history(
    history: &StaffingHistoryExpectation,
    trace: &forge_replay::Trace,
    state: &forge_kernel::GameState,
) -> Result<(), VerifyError> {
    for expected in history.steps {
        let Some(index) = expected.action_index.checked_sub(1) else {
            return Err(VerifyError::new("staffing checkpoint index is zero"));
        };
        let Some(step) = trace.steps.get(index) else {
            return Err(VerifyError::new("staffing checkpoint is missing"));
        };
        let before = if index == 0 {
            trace.initial_state.world.time
        } else {
            trace.steps[index - 1].observation.world_time
        };
        if step.action.definition_id != expected.definition
            || before != expected.before
            || step.observation.world_time != expected.after
        {
            return Err(VerifyError::new(
                "staffing checkpoint action or world time differs",
            ));
        }
        let actual: Vec<_> = step
            .events
            .iter()
            .filter(|event| staffing_mechanical_event(event))
            .cloned()
            .collect();
        let expected: Vec<_> = expected
            .events
            .iter()
            .copied()
            .map(StaffingEventExpectation::event)
            .collect();
        if actual != expected {
            return Err(VerifyError::new(
                "staffing movement, transfer, resource or time event order differs",
            ));
        }
    }
    let actual: Vec<_> = state.event_log.iter().filter(|event| matches!(&event.kind, forge_kernel::EventKind::ScheduledEventResolved { event_id, .. } if event_id == "lowsail.next_surge")).cloned().collect();
    let expected: Vec<_> = history
        .surge_resolution
        .map(|turn| staff_event(turn, StaffingEventKind::Surge { applied: false }).event())
        .into_iter()
        .collect();
    if actual != expected {
        return Err(VerifyError::new(
            "staffing absolute surge resolution differs",
        ));
    }
    let pending: Vec<_> = state
        .world
        .scheduled_events
        .iter()
        .filter(|event| event.id == "lowsail.next_surge")
        .collect();
    if pending.len() != usize::from(history.pending_surge)
        || pending
            .iter()
            .any(|event| event.due_time != 16 || event.event_kind != "deadline")
    {
        return Err(VerifyError::new("staffing absolute pending queue differs"));
    }
    for (npc, memory) in history.forbidden_memories {
        if state
            .world
            .npcs
            .get(*npc)
            .is_none_or(|actor| actor.memories.contains_key(*memory))
        {
            return Err(VerifyError::new(
                "staffing established a forbidden NPC memory",
            ));
        }
    }
    Ok(())
}

const fn staff_event(turn: u64, kind: StaffingEventKind) -> StaffingEventExpectation {
    StaffingEventExpectation { turn, kind }
}
const fn staff_step(
    action_index: usize,
    definition: &'static str,
    before: u64,
    after: u64,
    events: &'static [StaffingEventExpectation],
) -> StaffingStepExpectation {
    StaffingStepExpectation {
        action_index,
        definition,
        before,
        after,
        events,
    }
}
const STAFF_CHOICES: &[(&str, &str)] = &[
    ("lineage", "fenborn"),
    ("origin", "lowsail"),
    ("calling", "ledger-clerk"),
    ("value", "order"),
    ("burden", "indebted"),
    ("history", "saved-worker"),
];
const STAFF_OTHER_CHOICES: &[(&str, &str)] = &[
    ("lineage", "fenborn"),
    ("origin", "lowsail"),
    ("calling", "ledger-clerk"),
    ("value", "order"),
    ("burden", "indebted"),
    ("history", "stole-permit"),
];
const STAFF_START: ScenarioStartSpec = ScenarioStartSpec::Custom {
    name: "Crew comparison",
    choices: STAFF_CHOICES,
};
const STAFF_PREFIX12: [ScenarioStep; 12] = append_steps(
    &MARKET_PURCHASE_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
    ],
);
const STAFF_PREFIX13: [ScenarioStep; 13] =
    append_steps(&STAFF_PREFIX12, &[action!("fume_yards.fit_dust_filter")]);
const STAFF_STAFFED_STEPS: [ScenarioStep; 18] = append_steps(
    &STAFF_PREFIX13,
    &[
        action!("fume_yards.share_rescue_account"),
        action!("fume_yards.assign_brann_salvage"),
        action!("fume_yards.recover_staffed_filter"),
        action!("fume_yards.return_with_brann"),
        action!("fume_yards.load_cold_freight"),
    ],
);
const STAFF_ORDINARY_STEPS: [ScenarioStep; 20] = append_steps(
    &STAFF_PREFIX13,
    &[
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
        action!("fume_yards.report_with_daro"),
        action!("fume_yards.load_cold_freight"),
        action!("wait_tide"),
        action!("wait_tide"),
    ],
);
const STAFF_NO_ACCOUNT_STEPS: [ScenarioStep; 14] =
    append_steps(&STAFF_PREFIX13, &[action!("wait_tide")]);
const STAFF_NO_HELP_STEPS: [ScenarioStep; 14] = append_steps(
    &STAFF_PREFIX12,
    &[
        action!("fume_yards.share_rescue_account"),
        action!("wait_tide"),
    ],
);
const STAFF_WALKED_STEPS: [ScenarioStep; 16] = append_steps(
    &STAFF_PREFIX13,
    &[
        action!("fume_yards.share_rescue_account"),
        action!("fume_yards.assign_brann_salvage"),
        action!("fume_yards.leave_ash_hatch"),
    ],
);
const STAFF_CANCELLED_STEPS: [ScenarioStep; 18] = append_steps(
    &STAFF_WALKED_STEPS,
    &[
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.return_with_brann"),
    ],
);
const STAFF_WATER_STEPS: [ScenarioStep; 31] = append_steps(
    &STAFF_STAFFED_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("fume_yards.take_stock"),
        action!("fume_yards.press_repair_plugs"),
        action!("world.enter_aftermath"),
        action!("return.patch_stand"),
        action!("return.order_water_stand"),
        action!("return.fit_market_filter"),
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.take_market_cask"),
        action!("fume_yards.escort_market_cask"),
        action!("return.install_market_cask"),
        action!("return.draw_clean_water"),
    ],
);

const STAFF_DUST_RECIPE: ScenarioRecipeExpectation = batch_recipe(
    12,
    "fume_yards.fit_dust_filter",
    &[("fume_yards.filter", 1)],
    &[],
);
const STAFF_NO_CREW_MEMORIES: &[(&str, &str)] = &[
    (MARKET_BRANN, "fume_yards.salvage_assignment"),
    (MARKET_BRANN, "fume_yards.staffed_rack_lift"),
    (MARKET_BRANN, "fume_yards.returned_from_rack"),
];
const STAFF_NO_LIFT_MEMORIES: &[(&str, &str)] = &[
    (MARKET_BRANN, "fume_yards.staffed_rack_lift"),
    (MARKET_BRANN, "fume_yards.rack_report_paid"),
    (SALVAGE_DARO, "fume_yards.rack_cleared"),
    (SALVAGE_DARO, "fume_yards.rack_reported"),
];
const STAFF_POSITIONS: &[ScenarioNpcLocationExpectation] = &market_append::<_, 9>(
    AFTERMATH_NPC_LOCATIONS,
    &[
        market_position(MARKET_NESSA, MARKET_WORKSHOP),
        market_position(MARKET_BRANN, SALVAGE_BAY),
        market_position(MARKET_PERA, SALVAGE_BAY),
        market_position(SALVAGE_DARO, SALVAGE_ASH),
    ],
);
const STAFF_NO_FOREIGN_FACTS: &[ScenarioNpcKnowledgeAbsence] = &market_append::<_, 12>(
    MARKET_NO_REMOTE_TRADE,
    &[
        market_absent_knowledge(MARKET_NESSA, "fume_yards.rescue_account_heard"),
        market_absent_knowledge(MARKET_PERA, "fume_yards.rescue_account_heard"),
        market_absent_knowledge(SALVAGE_DARO, "fume_yards.rescue_account_heard"),
        market_absent_knowledge("oren_pell", "fume_yards.rescue_account_heard"),
        market_absent_knowledge(MARKET_NESSA, "fume_yards.rack_cleared"),
        market_absent_knowledge(MARKET_PERA, "fume_yards.rack_cleared"),
        market_absent_knowledge("oren_pell", "fume_yards.rack_cleared"),
        market_absent_knowledge(MARKET_BRANN, "fume_yards.market_cask"),
    ],
);
const STAFF_UNRESOLVED_FLAGS: &[(&str, &str)] = &market_append::<_, 17>(
    MARKET_NO_FIRE,
    &[
        (SALVAGE_BAY, "fume_yards.fuel_settled"),
        (SALVAGE_BAY, "fume_yards.kiln_closed"),
        (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
        (SALVAGE_ASH, "fume_yards.rack_cleared"),
        (SALVAGE_ASH, "fume_yards.rack_braced"),
        (SALVAGE_ASH, "fume_yards.report_paid"),
        (SALVAGE_ASH, "fume_yards.staffed_rack_lift_completed"),
        (MARKET_RETURN, "fume_yards.market_water_ordered"),
        (MARKET_RETURN, "fume_yards.market_filter_fitted"),
        (MARKET_RETURN, "fume_yards.market_cask_delivered"),
        (MARKET_RETURN, "fume_yards.market_cask_installed"),
        (MARKET_RETURN, "fume_yards.clean_water_drawn"),
    ],
);
const STAFF_BASE: ScenarioExpectations = ScenarioExpectations {
    staffing_history: None,
    final_location: SALVAGE_BAY,
    final_action_definition: "wait_tide",
    final_world_time: Some(14),
    final_observation_contains: "Your filter protects the kiln; load cold freight and close firing, or prepare a batch.",
    required_deeds: &["saved_worker"],
    required_npc_locations: STAFF_POSITIONS,
    required_location_flags: &[
        (SALVAGE_ASH, "fume_yards.collateral_settled"),
        (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
    ],
    forbidden_location_flags: &market_append::<_, 20>(
        STAFF_UNRESOLVED_FLAGS,
        &[
            (SALVAGE_ASH, "fume_yards.salvage_assignment_active"),
            (SALVAGE_ASH, "fume_yards.salvage_assignment_spent"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
        ],
    ),
    required_character_inventory: &[market_item("rope", 1)],
    forbidden_character_inventory: &market_append::<_, 11>(
        MARKET_NO_INTERMEDIATES,
        &["fume_yards.filter", "fume_yards.water_cask"],
    ),
    recipe_events: &[STAFF_DUST_RECIPE],
    required_npc_memories: &[
        batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
        batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 12),
    ],
    forbidden_npc_knowledge: &market_append::<_, 17>(
        STAFF_NO_FOREIGN_FACTS,
        &[
            market_absent_knowledge(MARKET_BRANN, "fume_yards.rescue_account_heard"),
            market_absent_knowledge(MARKET_BRANN, "fume_yards.rack_cleared"),
            market_absent_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared"),
            market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
            market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
        ],
    ),
    required_legal_definitions: &[
        "fume_yards.enter_ash_hatch",
        "fume_yards.share_rescue_account",
    ],
    forbidden_legal_definitions: &[
        "fume_yards.assign_brann_salvage",
        "fume_yards.recover_staffed_filter",
        "fume_yards.return_with_brann",
    ],
    ..MARKET_BASE
};

const fn staff_time(turn: u64, ticks: u64) -> StaffingEventExpectation {
    staff_event(turn, StaffingEventKind::Time { ticks })
}
const fn staff_coin(turn: u64, amount: i64) -> StaffingEventExpectation {
    staff_event(
        turn,
        StaffingEventKind::Resource {
            resource: "coin",
            amount,
        },
    )
}
const fn staff_player(turn: u64, from: &'static str, to: &'static str) -> StaffingEventExpectation {
    staff_event(turn, StaffingEventKind::PlayerMoved { from, to })
}
const fn staff_npc(
    turn: u64,
    npc: &'static str,
    from: &'static str,
    to: &'static str,
) -> StaffingEventExpectation {
    staff_event(turn, StaffingEventKind::NpcMoved { npc, from, to })
}
const fn staff_transfer(
    turn: u64,
    npc: &'static str,
    item: &'static str,
    count: u32,
) -> StaffingEventExpectation {
    staff_event(turn, StaffingEventKind::NpcTransfer { npc, item, count })
}
const STAFF_BUY_CHECK: StaffingStepExpectation = staff_step(
    10,
    "fume_yards.buy_collateral_filter",
    9,
    10,
    &[staff_coin(9, -4), staff_time(9, 1)],
);
const STAFF_FIT_CHECK: StaffingStepExpectation = staff_step(
    13,
    "fume_yards.fit_dust_filter",
    12,
    13,
    &[staff_time(12, 1)],
);
const STAFF_ACCOUNT_CHECK: StaffingStepExpectation = staff_step(
    14,
    "fume_yards.share_rescue_account",
    13,
    14,
    &[staff_time(13, 1)],
);
const STAFF_ASSIGN_CHECK: StaffingStepExpectation = staff_step(
    15,
    "fume_yards.assign_brann_salvage",
    14,
    15,
    &[
        staff_npc(14, MARKET_BRANN, SALVAGE_BAY, SALVAGE_ASH),
        staff_player(14, SALVAGE_BAY, SALVAGE_ASH),
        staff_time(14, 1),
    ],
);
const STAFF_WALK_CHECK: StaffingStepExpectation = staff_step(
    16,
    "fume_yards.leave_ash_hatch",
    15,
    16,
    &[
        staff_player(15, SALVAGE_ASH, SALVAGE_BAY),
        staff_time(15, 1),
        staff_event(16, StaffingEventKind::Surge { applied: false }),
    ],
);
const STAFF_WORK_CHECKS: &[StaffingStepExpectation] = &[
    STAFF_BUY_CHECK,
    STAFF_FIT_CHECK,
    STAFF_ACCOUNT_CHECK,
    STAFF_ASSIGN_CHECK,
    staff_step(
        16,
        "fume_yards.recover_staffed_filter",
        15,
        18,
        &[
            staff_transfer(15, SALVAGE_DARO, "fume_yards.filter", 1),
            staff_coin(15, 1),
            staff_time(15, 3),
            staff_event(18, StaffingEventKind::Surge { applied: false }),
        ],
    ),
    staff_step(
        17,
        "fume_yards.return_with_brann",
        18,
        19,
        &[
            staff_npc(18, MARKET_BRANN, SALVAGE_ASH, SALVAGE_BAY),
            staff_player(18, SALVAGE_ASH, SALVAGE_BAY),
            staff_time(18, 1),
        ],
    ),
    staff_step(
        18,
        "fume_yards.load_cold_freight",
        19,
        20,
        &[staff_coin(19, 3), staff_time(19, 1)],
    ),
];
const STAFF_WORK_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: STAFF_WORK_CHECKS,
    surge_resolution: Some(18),
    pending_surge: false,
    forbidden_memories: &[(SALVAGE_DARO, "fume_yards.rack_braced")],
};
const STAFF_ORDINARY_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &[
        STAFF_BUY_CHECK,
        STAFF_FIT_CHECK,
        staff_step(
            14,
            "fume_yards.enter_ash_hatch",
            13,
            14,
            &[
                staff_player(13, SALVAGE_BAY, SALVAGE_ASH),
                staff_time(13, 1),
            ],
        ),
        staff_step(
            15,
            "fume_yards.brace_rack",
            14,
            15,
            &[
                staff_event(
                    14,
                    StaffingEventKind::Resource {
                        resource: "stamina",
                        amount: -2,
                    },
                ),
                staff_time(14, 1),
            ],
        ),
        staff_step(
            16,
            "fume_yards.recover_braced_filter",
            15,
            16,
            &[
                staff_transfer(15, SALVAGE_DARO, "fume_yards.filter", 1),
                staff_time(15, 1),
                staff_event(16, StaffingEventKind::Surge { applied: false }),
            ],
        ),
        staff_step(
            17,
            "fume_yards.report_with_daro",
            16,
            17,
            &[
                staff_npc(16, SALVAGE_DARO, SALVAGE_ASH, SALVAGE_BAY),
                staff_player(16, SALVAGE_ASH, SALVAGE_BAY),
                staff_coin(16, 1),
                staff_time(16, 1),
            ],
        ),
        staff_step(
            18,
            "fume_yards.load_cold_freight",
            17,
            18,
            &[staff_coin(17, 3), staff_time(17, 1)],
        ),
        staff_step(19, "wait_tide", 18, 19, &[staff_time(18, 1)]),
        staff_step(20, "wait_tide", 19, 20, &[staff_time(19, 1)]),
    ],
    surge_resolution: Some(16),
    pending_surge: false,
    forbidden_memories: &market_append::<_, 4>(
        STAFF_NO_CREW_MEMORIES,
        &[(MARKET_BRANN, "fume_yards.rescue_account_heard")],
    ),
};
const STAFF_NO_ACCOUNT_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &[
        STAFF_BUY_CHECK,
        STAFF_FIT_CHECK,
        staff_step(14, "wait_tide", 13, 14, &[staff_time(13, 1)]),
    ],
    surge_resolution: None,
    pending_surge: true,
    forbidden_memories: &market_append::<_, 8>(
        &market_append::<_, 7>(STAFF_NO_CREW_MEMORIES, STAFF_NO_LIFT_MEMORIES),
        &[(MARKET_BRANN, "fume_yards.rescue_account_heard")],
    ),
};
const STAFF_NO_HELP_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &[
        STAFF_BUY_CHECK,
        staff_step(
            13,
            "fume_yards.share_rescue_account",
            12,
            13,
            &[staff_time(12, 1)],
        ),
        staff_step(14, "wait_tide", 13, 14, &[staff_time(13, 1)]),
    ],
    forbidden_memories: &market_append::<_, 8>(
        &market_append::<_, 7>(STAFF_NO_CREW_MEMORIES, STAFF_NO_LIFT_MEMORIES),
        &[(MARKET_BRANN, "fume_yards.dust_filter_fitted")],
    ),
    ..STAFF_NO_ACCOUNT_HISTORY
};
const STAFF_WALK_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &[
        STAFF_BUY_CHECK,
        STAFF_FIT_CHECK,
        STAFF_ACCOUNT_CHECK,
        STAFF_ASSIGN_CHECK,
        STAFF_WALK_CHECK,
    ],
    surge_resolution: Some(16),
    pending_surge: false,
    forbidden_memories: &market_append::<_, 5>(
        STAFF_NO_LIFT_MEMORIES,
        &[(MARKET_BRANN, "fume_yards.returned_from_rack")],
    ),
};
const STAFF_CANCEL_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &[
        STAFF_BUY_CHECK,
        STAFF_FIT_CHECK,
        STAFF_ACCOUNT_CHECK,
        STAFF_ASSIGN_CHECK,
        STAFF_WALK_CHECK,
        staff_step(
            17,
            "fume_yards.enter_ash_hatch",
            16,
            17,
            &[
                staff_player(16, SALVAGE_BAY, SALVAGE_ASH),
                staff_time(16, 1),
            ],
        ),
        staff_step(
            18,
            "fume_yards.return_with_brann",
            17,
            18,
            &[
                staff_npc(17, MARKET_BRANN, SALVAGE_ASH, SALVAGE_BAY),
                staff_player(17, SALVAGE_ASH, SALVAGE_BAY),
                staff_time(17, 1),
            ],
        ),
    ],
    forbidden_memories: STAFF_NO_LIFT_MEMORIES,
    ..STAFF_WALK_HISTORY
};
const STAFF_WATER_HISTORY: StaffingHistoryExpectation = StaffingHistoryExpectation {
    steps: &market_append::<_, 11>(
        STAFF_WORK_CHECKS,
        &[
            staff_step(
                20,
                "fume_yards.take_stock",
                21,
                22,
                &[
                    staff_transfer(21, MARKET_NESSA, "fume_yards.clay", 2),
                    staff_transfer(21, MARKET_NESSA, "fume_yards.mesh", 1),
                    staff_time(21, 1),
                ],
            ),
            staff_step(
                28,
                "fume_yards.take_market_cask",
                29,
                30,
                &[
                    staff_transfer(29, MARKET_PERA, "fume_yards.water_cask", 1),
                    staff_time(29, 1),
                ],
            ),
            staff_step(
                29,
                "fume_yards.escort_market_cask",
                30,
                31,
                &[
                    staff_npc(30, MARKET_PERA, SALVAGE_BAY, MARKET_RETURN),
                    staff_player(30, SALVAGE_BAY, MARKET_RETURN),
                    staff_time(30, 1),
                ],
            ),
            staff_step(
                31,
                "return.draw_clean_water",
                32,
                33,
                &[
                    staff_event(
                        32,
                        StaffingEventKind::Resource {
                            resource: "stamina",
                            amount: 2,
                        },
                    ),
                    staff_time(32, 1),
                ],
            ),
        ],
    ),
    ..STAFF_WORK_HISTORY
};

const STAFF_WORK_FLAGS: &[(&str, &str)] = &[
    (SALVAGE_ASH, "fume_yards.collateral_settled"),
    (SALVAGE_ASH, "fume_yards.rack_rear_open"),
    (SALVAGE_ASH, "fume_yards.rack_cleared"),
    (SALVAGE_ASH, "fume_yards.report_paid"),
    (SALVAGE_ASH, "fume_yards.salvage_assignment_spent"),
    (SALVAGE_ASH, "fume_yards.staffed_rack_lift_completed"),
    (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
    (SALVAGE_BAY, "fume_yards.kiln_closed"),
    (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
    (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
];
const STAFF_WORK_FORBIDDEN_FLAGS: &[(&str, &str)] = &market_append::<_, 13>(
    MARKET_NO_FIRE,
    &[
        (SALVAGE_BAY, "fume_yards.fuel_settled"),
        (SALVAGE_ASH, "fume_yards.rack_braced"),
        (SALVAGE_ASH, "fume_yards.salvage_assignment_active"),
        (MARKET_RETURN, "fume_yards.market_water_ordered"),
        (MARKET_RETURN, "fume_yards.market_filter_fitted"),
        (MARKET_RETURN, "fume_yards.market_cask_delivered"),
        (MARKET_RETURN, "fume_yards.market_cask_installed"),
        (MARKET_RETURN, "fume_yards.clean_water_drawn"),
    ],
);
const STAFF_WORK_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] = &[
    market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 9),
    market_knowledge(MARKET_BRANN, "fume_yards.rescue_account_heard", 13),
    market_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared", 15),
    market_knowledge(MARKET_BRANN, "fume_yards.rack_cleared", 15),
];
const STAFF_WORK_MEMORIES: &[ScenarioNpcMemoryExpectation] = &[
    batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
    batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 12),
    batch_memory(MARKET_BRANN, "fume_yards.rescue_account_heard", 13),
    batch_memory(MARKET_BRANN, "fume_yards.salvage_assignment", 14),
    batch_memory(MARKET_BRANN, "fume_yards.staffed_rack_lift", 15),
    batch_memory(MARKET_BRANN, "fume_yards.rack_report_paid", 15),
    batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 15),
    batch_memory(SALVAGE_DARO, "fume_yards.rack_reported", 15),
    batch_memory(MARKET_BRANN, "fume_yards.returned_from_rack", 18),
    batch_memory(MARKET_BRANN, "fume_yards.kiln_freight_paid", 19),
    batch_memory(MARKET_BRANN, "fume_yards.cold_work_chosen", 19),
];
const STAFF_WORK_EXPECTED: ScenarioExpectations = ScenarioExpectations {
    staffing_history: Some(&STAFF_WORK_HISTORY),
    final_action_definition: "fume_yards.load_cold_freight",
    final_world_time: Some(20),
    final_observation_contains: "Brann pays three coins; cold work closes firing.",
    required_character_inventory: &[market_item("rope", 1), market_item("fume_yards.filter", 1)],
    required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
    forbidden_character_inventory: &market_append::<_, 10>(
        MARKET_NO_INTERMEDIATES,
        &["fume_yards.water_cask"],
    ),
    required_location_flags: STAFF_WORK_FLAGS,
    forbidden_location_flags: STAFF_WORK_FORBIDDEN_FLAGS,
    required_npc_knowledge: STAFF_WORK_KNOWLEDGE,
    required_npc_memories: STAFF_WORK_MEMORIES,
    forbidden_npc_knowledge: &market_append::<_, 14>(
        STAFF_NO_FOREIGN_FACTS,
        &[
            market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
            market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
        ],
    ),
    required_npc_inventory: &[
        MARKET_ALL_STOCK[1],
        MARKET_ALL_STOCK[2],
        MARKET_ALL_STOCK[3],
        MARKET_ALL_STOCK[4],
    ],
    forbidden_npc_inventory: &[ScenarioNpcInventoryAbsence {
        npc: SALVAGE_DARO,
        item: "fume_yards.filter",
    }],
    required_legal_definitions: &["wait_tide", "fume_yards.enter_ash_hatch"],
    forbidden_legal_definitions: &[
        "fume_yards.share_rescue_account",
        "fume_yards.assign_brann_salvage",
        "fume_yards.recover_staffed_filter",
        "fume_yards.return_with_brann",
        "fume_yards.load_cold_freight",
        "fume_yards.ignite_batch",
        "fume_yards.prepare_charge",
    ],
    ..STAFF_BASE
};
const STAFF_STAFFED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-staffed",
    claim_id: "milestone-2.staffing.staffed",
    start: STAFF_START,
    seed: 71,
    steps: &STAFF_STAFFED_STEPS,
    expectations: STAFF_WORK_EXPECTED,
};
const STAFF_ORDINARY_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-ordinary",
    claim_id: "milestone-2.staffing.ordinary",
    steps: &STAFF_ORDINARY_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_ORDINARY_HISTORY),
        final_action_definition: "wait_tide",
        final_observation_contains: "Cold freight is finished; the unfired kiln stays closed.",
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 1)],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(MARKET_NESSA, MARKET_WORKSHOP),
                market_position(MARKET_BRANN, SALVAGE_BAY),
                market_position(MARKET_PERA, SALVAGE_BAY),
                market_position(SALVAGE_DARO, SALVAGE_BAY),
            ],
        ),
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.kiln_closed"),
            (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
            (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
        ],
        forbidden_location_flags: &market_append::<_, 9>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_BAY, "fume_yards.fuel_settled"),
                (SALVAGE_ASH, "fume_yards.salvage_assignment_active"),
                (SALVAGE_ASH, "fume_yards.salvage_assignment_spent"),
                (SALVAGE_ASH, "fume_yards.staffed_rack_lift_completed"),
            ],
        ),
        required_npc_knowledge: &[
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 9),
            market_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared", 15),
            ScenarioNpcKnowledgeExpectation {
                npc: MARKET_BRANN,
                knowledge_id: "fume_yards.rack_cleared",
                provenance: ScenarioKnowledgeProvenance::Told { by: SALVAGE_DARO },
                turn: 16,
            },
        ],
        forbidden_npc_knowledge: &market_append::<_, 15>(
            STAFF_NO_FOREIGN_FACTS,
            &[
                market_absent_knowledge(MARKET_BRANN, "fume_yards.rescue_account_heard"),
                market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
                market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
            ],
        ),
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
            batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 12),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_braced", 14),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 15),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_reported", 16),
            batch_memory(MARKET_BRANN, "fume_yards.rack_report_paid", 16),
            batch_memory(MARKET_BRANN, "fume_yards.kiln_freight_paid", 17),
            batch_memory(MARKET_BRANN, "fume_yards.cold_work_chosen", 17),
        ],
        ..STAFF_WORK_EXPECTED
    },
    ..STAFF_STAFFED_SPEC
};
const STAFF_OTHER_HISTORY_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-other-history",
    claim_id: "milestone-2.staffing.other-history",
    start: ScenarioStartSpec::Custom {
        name: "Crew comparison",
        choices: STAFF_OTHER_CHOICES,
    },
    expectations: ScenarioExpectations {
        required_deeds: &["stole_permit"],
        ..STAFF_ORDINARY_SPEC.expectations
    },
    ..STAFF_ORDINARY_SPEC
};
const STAFF_NO_ACCOUNT_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-no-account",
    claim_id: "milestone-2.staffing.no-account",
    steps: &STAFF_NO_ACCOUNT_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_NO_ACCOUNT_HISTORY),
        ..STAFF_BASE
    },
    ..STAFF_STAFFED_SPEC
};
const STAFF_NO_HELP_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-no-help",
    claim_id: "milestone-2.staffing.no-help",
    steps: &STAFF_NO_HELP_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_NO_HELP_HISTORY),
        final_observation_contains: "Brann needs an installed dust filter before he can help at the rack.",
        recipe_events: &[],
        required_location_flags: &[(SALVAGE_ASH, "fume_yards.collateral_settled")],
        forbidden_location_flags: &market_append::<_, 21>(
            STAFF_BASE.forbidden_location_flags,
            &[(SALVAGE_BAY, "fume_yards.dust_filter_fitted")],
        ),
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.filter", 1),
        ],
        forbidden_character_inventory: &market_append::<_, 10>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.water_cask"],
        ),
        required_npc_knowledge: &[
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 9),
            market_knowledge(MARKET_BRANN, "fume_yards.rescue_account_heard", 12),
        ],
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
            batch_memory(MARKET_BRANN, "fume_yards.rescue_account_heard", 12),
        ],
        forbidden_npc_knowledge: &market_append::<_, 16>(
            STAFF_NO_FOREIGN_FACTS,
            &[
                market_absent_knowledge(MARKET_BRANN, "fume_yards.rack_cleared"),
                market_absent_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared"),
                market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
                market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
            ],
        ),
        required_legal_definitions: &["fume_yards.fit_dust_filter", "fume_yards.enter_ash_hatch"],
        forbidden_legal_definitions: &[
            "fume_yards.share_rescue_account",
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
        ],
        ..STAFF_BASE
    },
    ..STAFF_STAFFED_SPEC
};
const STAFF_WALKED_AWAY_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-walked-away",
    claim_id: "milestone-2.staffing.walked-away",
    steps: &STAFF_WALKED_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_WALK_HISTORY),
        final_world_time: Some(16),
        final_action_definition: "fume_yards.leave_ash_hatch",
        final_observation_contains: "Brann is staffing Ash Beds; bring him back through the hatch.",
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_ASH, "fume_yards.salvage_assignment_active"),
            (SALVAGE_ASH, "fume_yards.salvage_assignment_spent"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
        ],
        forbidden_location_flags: STAFF_UNRESOLVED_FLAGS,
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(MARKET_NESSA, MARKET_WORKSHOP),
                market_position(MARKET_BRANN, SALVAGE_ASH),
                market_position(MARKET_PERA, SALVAGE_BAY),
                market_position(SALVAGE_DARO, SALVAGE_ASH),
            ],
        ),
        required_npc_knowledge: &[
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 9),
            market_knowledge(MARKET_BRANN, "fume_yards.rescue_account_heard", 13),
        ],
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
            batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 12),
            batch_memory(MARKET_BRANN, "fume_yards.rescue_account_heard", 13),
            batch_memory(MARKET_BRANN, "fume_yards.salvage_assignment", 14),
        ],
        forbidden_npc_knowledge: &market_append::<_, 16>(
            STAFF_NO_FOREIGN_FACTS,
            &[
                market_absent_knowledge(MARKET_BRANN, "fume_yards.rack_cleared"),
                market_absent_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared"),
                market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
                market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
            ],
        ),
        required_legal_definitions: &["fume_yards.enter_ash_hatch"],
        forbidden_legal_definitions: &[
            "fume_yards.take_fuel",
            "fume_yards.load_cold_freight",
            "fume_yards.assign_brann_salvage",
            "fume_yards.return_with_brann",
        ],
        ..STAFF_BASE
    },
    ..STAFF_STAFFED_SPEC
};
const STAFF_CANCELLED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-cancelled",
    claim_id: "milestone-2.staffing.cancelled",
    steps: &STAFF_CANCELLED_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_CANCEL_HISTORY),
        final_world_time: Some(18),
        final_action_definition: "fume_yards.return_with_brann",
        final_observation_contains: "Brann returns with you to Kiln Bay. His one rack assignment is spent.",
        required_npc_locations: STAFF_POSITIONS,
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_ASH, "fume_yards.salvage_assignment_spent"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
        ],
        forbidden_location_flags: &market_append::<_, 18>(
            STAFF_UNRESOLVED_FLAGS,
            &[(SALVAGE_ASH, "fume_yards.salvage_assignment_active")],
        ),
        required_npc_memories: &market_append::<_, 5>(
            STAFF_WALKED_AWAY_SPEC.expectations.required_npc_memories,
            &[batch_memory(
                MARKET_BRANN,
                "fume_yards.returned_from_rack",
                17,
            )],
        ),
        required_legal_definitions: &[
            "fume_yards.enter_ash_hatch",
            "fume_yards.take_fuel",
            "fume_yards.load_cold_freight",
        ],
        forbidden_legal_definitions: &[
            "fume_yards.share_rescue_account",
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
        ],
        ..STAFF_WALKED_AWAY_SPEC.expectations
    },
    ..STAFF_STAFFED_SPEC
};
const STAFF_WATER_COMPOSED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-crew-water-composed",
    claim_id: "milestone-2.staffing.water-composed",
    steps: &STAFF_WATER_STEPS,
    expectations: ScenarioExpectations {
        staffing_history: Some(&STAFF_WATER_HISTORY),
        final_location: MARKET_RETURN,
        final_world_time: Some(33),
        final_action_definition: "return.draw_clean_water",
        final_observation_contains: "Oren supplies one clean-water ration, restoring two stamina.",
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 5)],
        required_character_inventory: &[market_item("rope", 1)],
        forbidden_character_inventory: &market_append::<_, 11>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.filter", "fume_yards.water_cask"],
        ),
        required_location_flags: &market_append::<_, 17>(
            STAFF_WORK_FLAGS,
            &[
                (MARKET_WORKSHOP, "fume_yards.stock_given"),
                (MARKET_RETURN, "fume_yards.stand_patched"),
                (MARKET_RETURN, "fume_yards.market_water_ordered"),
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_cask_delivered"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        forbidden_location_flags: &market_append::<_, 9>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_BAY, "fume_yards.fuel_settled"),
                (SALVAGE_ASH, "fume_yards.rack_braced"),
                (SALVAGE_ASH, "fume_yards.salvage_assignment_active"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
            ],
        ),
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(MARKET_NESSA, MARKET_WORKSHOP),
                market_position(MARKET_BRANN, SALVAGE_BAY),
                market_position(MARKET_PERA, MARKET_RETURN),
                market_position(SALVAGE_DARO, SALVAGE_ASH),
            ],
        ),
        required_npc_inventory: &[market_stock(MARKET_BRANN, "fume_yards.fuel", 1)],
        forbidden_npc_inventory: &[
            ScenarioNpcInventoryAbsence {
                npc: SALVAGE_DARO,
                item: "fume_yards.filter",
            },
            ScenarioNpcInventoryAbsence {
                npc: MARKET_PERA,
                item: "fume_yards.water_cask",
            },
            MARKET_EMPTY_NESSA[0],
            MARKET_EMPTY_NESSA[1],
        ],
        recipe_events: &[
            STAFF_DUST_RECIPE,
            batch_recipe(
                22,
                "fume_yards.press_repair_plugs",
                PILOT_INPUTS,
                &[("fume_yards.repair_lot", 1)],
            ),
            batch_recipe(
                24,
                "fume_yards.patch_stand",
                &[("fume_yards.repair_lot", 1)],
                &[],
            ),
            batch_recipe(
                26,
                "fume_yards.fit_market_filter",
                &[("fume_yards.filter", 1)],
                &[],
            ),
            batch_recipe(
                31,
                "fume_yards.install_market_cask",
                &[("fume_yards.water_cask", 1)],
                &[],
            ),
        ],
        required_npc_knowledge: &market_append::<_, 9>(
            STAFF_WORK_KNOWLEDGE,
            &[
                market_knowledge("oren_pell", "fume_yards.stand_patched", 24),
                market_knowledge("oren_pell", "fume_yards.market_water_ordered", 25),
                market_knowledge(MARKET_PERA, "fume_yards.market_cask", 30),
                ScenarioNpcKnowledgeExpectation {
                    npc: "oren_pell",
                    knowledge_id: "fume_yards.market_cask",
                    provenance: ScenarioKnowledgeProvenance::Told { by: MARKET_PERA },
                    turn: 30,
                },
                market_knowledge("oren_pell", "fume_yards.clean_water_supplied", 32),
            ],
        ),
        required_npc_memories: &market_append::<_, 20>(
            STAFF_WORK_MEMORIES,
            &[
                batch_memory(MARKET_NESSA, "fume_yards.stock_handed_over", 21),
                batch_memory(MARKET_NESSA, "fume_yards.repair_plugs_pressed", 22),
                batch_memory("oren_pell", "fume_yards.stand_patched", 24),
                batch_memory("oren_pell", "fume_yards.market_water_ordered", 25),
                batch_memory(MARKET_PERA, "fume_yards.market_cask_handed_over", 29),
                batch_memory(MARKET_PERA, "fume_yards.market_cask_escorted", 30),
                batch_memory("oren_pell", "fume_yards.market_cask_arrived", 30),
                batch_memory("oren_pell", "fume_yards.market_cask_installed", 31),
                batch_memory("oren_pell", "fume_yards.clean_water_supplied", 32),
            ],
        ),
        forbidden_npc_knowledge: &market_append::<_, 14>(
            STAFF_NO_FOREIGN_FACTS,
            &[
                market_absent_knowledge(MARKET_NESSA, "fume_yards.market_cask"),
                market_absent_knowledge(SALVAGE_DARO, "fume_yards.market_cask"),
            ],
        ),
        required_legal_definitions: &["return.visit_workshop"],
        forbidden_legal_definitions: &[
            "return.draw_clean_water",
            "return.fit_market_filter",
            "return.install_market_cask",
            "return.sell_filter",
            "fume_yards.recover_staffed_filter",
        ],
        ..STAFF_WORK_EXPECTED
    },
    ..STAFF_STAFFED_SPEC
};

#[cfg(test)]
mod staffing_tests {
    use super::*;
    use forge_kernel::{EventKind, KnowledgeProvenance};

    const SPECS: &[ScenarioSpec] = &[
        STAFF_STAFFED_SPEC,
        STAFF_ORDINARY_SPEC,
        STAFF_OTHER_HISTORY_SPEC,
        STAFF_NO_ACCOUNT_SPEC,
        STAFF_NO_HELP_SPEC,
        STAFF_WALKED_AWAY_SPEC,
        STAFF_CANCELLED_SPEC,
        STAFF_WATER_COMPOSED_SPEC,
    ];

    fn content() -> CompiledContent {
        forge_content::parse_and_compile_production(include_str!(
            "../../../content/split-tide.json"
        ))
        .unwrap()
    }

    #[test]
    fn staffing_eight_literal_production_paths_bind_time_stock_and_character_tradeoffs() {
        let content = content();
        validate_specs(SPECS).unwrap();
        let legacy: Vec<_> = SCENARIOS
            .iter()
            .filter(|spec| spec.expectations.staffing_history.is_none())
            .collect();
        assert_eq!(legacy.len(), 47);
        for spec in legacy {
            assert!(
                serde_json::to_value(spec.expectations)
                    .unwrap()
                    .get("staffing_history")
                    .is_none(),
                "legacy binding gained a staffing field"
            );
        }
        for spec in SPECS {
            let session =
                run(spec, &content).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
            assert_eq!(session.trace().steps.len(), spec.steps.len());
            assert_eq!(
                forge_replay::verify(session.trace(), &content).unwrap(),
                *session.state()
            );
            assert_eq!(session.state().entropy.cursor, 0);
        }
        let staffed = run(&STAFF_STAFFED_SPEC, &content).unwrap();
        let ordinary = run(&STAFF_ORDINARY_SPEC, &content).unwrap();
        let other = run(&STAFF_OTHER_HISTORY_SPEC, &content).unwrap();
        assert_eq!(
            (staffed.trace().steps.len(), staffed.state().world.time),
            (18, 20)
        );
        assert_eq!(
            (ordinary.trace().steps.len(), ordinary.state().world.time),
            (20, 20)
        );
        assert_eq!(
            staffed.state().character.inventory,
            ordinary.state().character.inventory
        );
        assert_eq!(
            ordinary.state().character.resources,
            other.state().character.resources
        );
        assert_eq!(
            ordinary.state().character.inventory,
            other.state().character.inventory
        );
        assert_eq!(staffed.state().character.resources["stamina"], 3);
        assert_eq!(ordinary.state().character.resources["stamina"], 1);
        assert_eq!(
            staffed.state().world.npcs[MARKET_BRANN].knowledge["fume_yards.rack_cleared"]
                .provenance,
            KnowledgeProvenance::Witnessed
        );
        assert_eq!(
            ordinary.state().world.npcs[MARKET_BRANN].knowledge["fume_yards.rack_cleared"]
                .provenance,
            KnowledgeProvenance::Told {
                by: SALVAGE_DARO.into()
            }
        );
    }

    #[test]
    fn staffing_history_oracle_rejects_movement_transfer_payment_time_and_deadline_corruption() {
        let content = content();
        let session = run(&STAFF_STAFFED_SPEC, &content).unwrap();
        for defect in [
            "move missing",
            "move order",
            "move owner",
            "move destination",
            "move time",
            "transfer count",
            "transfer owner",
            "transfer time",
            "double wage",
            "short lift",
            "wrong action",
            "wrong end",
            "surge time",
            "surge applies",
            "surge missing",
            "surge duplicate",
            "pending resurrection",
        ] {
            let mut trace = session.trace().clone();
            let mut state = session.state().clone();
            match defect {
                "move missing" => trace.steps[14]
                    .events
                    .retain(|event| !matches!(event.kind, EventKind::NpcMoved { .. })),
                "move order" => {
                    let npc = trace.steps[14]
                        .events
                        .iter()
                        .position(|event| matches!(event.kind, EventKind::NpcMoved { .. }))
                        .unwrap();
                    let player = trace.steps[14]
                        .events
                        .iter()
                        .position(|event| matches!(event.kind, EventKind::Moved { .. }))
                        .unwrap();
                    trace.steps[14].events.swap(npc, player);
                }
                "move owner" | "move destination" | "move time" => {
                    let event = trace.steps[14]
                        .events
                        .iter_mut()
                        .find(|event| matches!(event.kind, EventKind::NpcMoved { .. }))
                        .unwrap();
                    if defect == "move time" {
                        event.turn += 1;
                    } else if let EventKind::NpcMoved { npc, to, .. } = &mut event.kind {
                        if defect == "move owner" {
                            *npc = MARKET_PERA.into();
                        } else {
                            *to = MARKET_WORKSHOP.into();
                        }
                    }
                }
                "transfer count" | "transfer owner" | "transfer time" => {
                    let event = trace.steps[15]
                        .events
                        .iter_mut()
                        .find(|event| {
                            matches!(event.kind, EventKind::NpcItemTransferredToCharacter { .. })
                        })
                        .unwrap();
                    if defect == "transfer time" {
                        event.turn = 18;
                    } else if let EventKind::NpcItemTransferredToCharacter { npc, count, .. } =
                        &mut event.kind
                    {
                        if defect == "transfer count" {
                            *count = 2;
                        } else {
                            *npc = MARKET_PERA.into();
                        }
                    }
                }
                "double wage" => {
                    let event = trace.steps[15]
                        .events
                        .iter_mut()
                        .find(|event| matches!(event.kind, EventKind::ResourceAdjusted { .. }))
                        .unwrap();
                    if let EventKind::ResourceAdjusted { amount, .. } = &mut event.kind {
                        *amount = 2;
                    }
                }
                "short lift" => {
                    let event = trace.steps[15]
                        .events
                        .iter_mut()
                        .find(|event| matches!(event.kind, EventKind::TimeAdvanced { .. }))
                        .unwrap();
                    if let EventKind::TimeAdvanced { ticks } = &mut event.kind {
                        *ticks = 1;
                    }
                }
                "wrong action" => {
                    trace.steps[15].action.definition_id = "fume_yards.recover_braced_filter".into()
                }
                "wrong end" => trace.steps[15].observation.world_time = 16,
                "surge time" | "surge applies" => {
                    let event = state
                        .event_log
                        .iter_mut()
                        .find(|event| {
                            matches!(event.kind, EventKind::ScheduledEventResolved { .. })
                        })
                        .unwrap();
                    if defect == "surge time" {
                        event.turn = 16;
                    } else if let EventKind::ScheduledEventResolved { applied, .. } =
                        &mut event.kind
                    {
                        *applied = true;
                    }
                }
                "surge missing" => state.event_log.retain(|event| {
                    !matches!(event.kind, EventKind::ScheduledEventResolved { .. })
                }),
                "surge duplicate" => {
                    let event = state
                        .event_log
                        .iter()
                        .find(|event| {
                            matches!(event.kind, EventKind::ScheduledEventResolved { .. })
                        })
                        .unwrap()
                        .clone();
                    state.event_log.push(event);
                }
                "pending resurrection" => {
                    state
                        .world
                        .scheduled_events
                        .push(forge_kernel::ScheduledEvent {
                            id: "lowsail.next_surge".into(),
                            event_kind: "deadline".into(),
                            due_time: 16,
                        })
                }
                _ => unreachable!(),
            }
            assert!(
                validate_staffing_history(&STAFF_WORK_HISTORY, &trace, &state).is_err(),
                "accepted {defect}"
            );
        }
    }

    #[test]
    fn staffing_negative_oracles_reject_false_recognition_foreman_return_and_resources() {
        let content = content();
        let no_account = run(&STAFF_NO_ACCOUNT_SPEC, &content).unwrap();
        let no_help = run(&STAFF_NO_HELP_SPEC, &content).unwrap();
        let walked = run(&STAFF_WALKED_AWAY_SPEC, &content).unwrap();
        let cancelled = run(&STAFF_CANCELLED_SPEC, &content).unwrap();
        assert!(
            !no_account.state().world.npcs[MARKET_BRANN]
                .knowledge
                .contains_key("fume_yards.rescue_account_heard")
        );
        assert!(
            !no_help.state().world.npcs[MARKET_BRANN]
                .memories
                .contains_key("fume_yards.dust_filter_fitted")
        );
        assert_eq!(
            no_help.state().world.npcs[MARKET_BRANN].knowledge["fume_yards.rescue_account_heard"]
                .subject,
            "Brann heard the player's account of saving a worker."
        );
        validate_npc_knowledge(
            no_account.state(),
            STAFF_NO_ACCOUNT_SPEC.expectations.required_npc_knowledge,
            STAFF_NO_ACCOUNT_SPEC.expectations.forbidden_npc_knowledge,
        )
        .unwrap();
        let mut false_account = no_account.state().clone();
        let mut account =
            no_help.state().world.npcs[MARKET_BRANN].knowledge["fume_yards.rescue_account_heard"]
                .clone();
        account.turn = 13;
        false_account
            .world
            .npcs
            .get_mut(MARKET_BRANN)
            .unwrap()
            .knowledge
            .insert("fume_yards.rescue_account_heard".into(), account);
        let error = validate_npc_knowledge(
            &false_account,
            STAFF_NO_ACCOUNT_SPEC.expectations.required_npc_knowledge,
            STAFF_NO_ACCOUNT_SPEC.expectations.forbidden_npc_knowledge,
        )
        .unwrap_err();
        assert!(error.to_string().contains("established forbidden"));

        validate_staffing_history(&STAFF_NO_HELP_HISTORY, no_help.trace(), no_help.state())
            .unwrap();
        let mut false_assignment = no_help.state().clone();
        let mut assignment =
            walked.state().world.npcs[MARKET_BRANN].memories["fume_yards.salvage_assignment"]
                .clone();
        assignment.turn = 13;
        false_assignment
            .world
            .npcs
            .get_mut(MARKET_BRANN)
            .unwrap()
            .memories
            .insert("fume_yards.salvage_assignment".into(), assignment);
        let error =
            validate_staffing_history(&STAFF_NO_HELP_HISTORY, no_help.trace(), &false_assignment)
                .unwrap_err();
        assert!(error.to_string().contains("forbidden NPC memory"));

        let mut invented = cancelled.state().clone();
        let memory =
            invented.world.npcs[MARKET_BRANN].memories["fume_yards.dust_filter_fitted"].clone();
        invented
            .world
            .npcs
            .get_mut(MARKET_BRANN)
            .unwrap()
            .memories
            .insert("fume_yards.rack_report_paid".into(), memory);
        assert!(
            validate_staffing_history(&STAFF_CANCEL_HISTORY, cancelled.trace(), &invented).is_err()
        );
        let mut returned = walked.state().clone();
        returned.world.npcs.get_mut(MARKET_BRANN).unwrap().location = SALVAGE_BAY.into();
        assert!(
            validate_npc_locations(
                &returned,
                STAFF_WALKED_AWAY_SPEC.expectations.required_npc_locations
            )
            .is_err()
        );
        let mut wrong = run(&STAFF_STAFFED_SPEC, &content).unwrap().state().clone();
        wrong.character.resources.insert("stamina".into(), 1);
        assert!(
            validate_character_resources(&wrong, STAFF_WORK_EXPECTED.required_character_resources)
                .is_err()
        );
        wrong
            .character
            .inventory
            .insert("fume_yards.filter".into(), 2);
        assert!(
            validate_inventory(
                &wrong,
                STAFF_WORK_EXPECTED.required_character_inventory,
                STAFF_WORK_EXPECTED.forbidden_npc_inventory
            )
            .is_err()
        );
        let mut bad = STAFF_STAFFED_SPEC;
        bad.expectations.staffing_history = Some(&STAFF_ORDINARY_HISTORY);
        assert!(
            validate_specs(&[bad]).is_err(),
            "step identities must bind the typed history"
        );
    }

    #[test]
    fn staffing_water_oracle_requires_exact_consumption_cask_source_and_single_ration() {
        let content = content();
        let session = run(&STAFF_WATER_COMPOSED_SPEC, &content).unwrap();
        assert_eq!(
            (session.trace().steps.len(), session.state().world.time),
            (31, 33)
        );
        let state = session.state();
        for defect in [
            "lost recipe",
            "retained filter",
            "restored cask",
            "extra stamina",
            "wrong source",
            "late source",
        ] {
            let mut wrong = state.clone();
            match defect {
                "lost recipe" => {
                    let index = wrong.event_log.iter().position(|event| matches!(&event.kind, EventKind::RecipeApplied { recipe, .. } if recipe == "fume_yards.fit_market_filter")).unwrap();
                    wrong.event_log.remove(index);
                    assert!(
                        validate_recipe_events(
                            &wrong,
                            STAFF_WATER_COMPOSED_SPEC.expectations.recipe_events
                        )
                        .is_err()
                    );
                }
                "retained filter" | "restored cask" => {
                    let item = if defect == "retained filter" {
                        "fume_yards.filter"
                    } else {
                        "fume_yards.water_cask"
                    };
                    wrong.character.inventory.insert(item.into(), 1);
                    assert!(
                        validate_forbidden_inventory(
                            &wrong,
                            STAFF_WATER_COMPOSED_SPEC
                                .expectations
                                .forbidden_character_inventory
                        )
                        .is_err()
                    );
                }
                "extra stamina" => {
                    wrong.character.resources.insert("stamina".into(), 7);
                    assert!(
                        validate_character_resources(
                            &wrong,
                            STAFF_WATER_COMPOSED_SPEC
                                .expectations
                                .required_character_resources
                        )
                        .is_err()
                    );
                }
                "wrong source" | "late source" => {
                    let fact = wrong
                        .world
                        .npcs
                        .get_mut("oren_pell")
                        .unwrap()
                        .knowledge
                        .get_mut("fume_yards.market_cask")
                        .unwrap();
                    if defect == "late source" {
                        fact.turn = 31;
                    } else {
                        fact.provenance = KnowledgeProvenance::Witnessed;
                    }
                    assert!(
                        validate_npc_knowledge(
                            &wrong,
                            STAFF_WATER_COMPOSED_SPEC
                                .expectations
                                .required_npc_knowledge,
                            &[]
                        )
                        .is_err()
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
