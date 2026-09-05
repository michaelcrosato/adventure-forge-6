// Literal cycle-37 claims. Expected records and event order are authored independently.
#[derive(Clone, Copy, Debug, Serialize)]
struct ColdShiftHistory {
    steps: &'static [ColdStep],
    records: &'static [ColdRecord],
    stocks: &'static [ColdStock],
    player_items: &'static [(&'static str, u32)],
    surge: Option<u64>,
}
#[derive(Clone, Copy, Debug, Serialize)]
struct ColdStock {
    npc: &'static str,
    items: &'static [(&'static str, u32)],
}
#[derive(Clone, Copy, Debug, Serialize)]
struct ColdRecord {
    npc: &'static str,
    id: &'static str,
    subject: &'static str,
    provenance: ScenarioKnowledgeProvenance,
    turn: u64,
    memory: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
struct ColdStep {
    index: usize,
    definition: &'static str,
    before: u64,
    after: u64,
    events: &'static [ColdEvent],
}
#[derive(Clone, Copy, Debug, Serialize)]
struct ColdEvent {
    turn: u64,
    kind: ColdEventKind,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ColdEventKind {
    Mechanical {
        value: StaffingEventKind,
    },
    Recipe {
        recipe: &'static str,
        inputs: &'static [(&'static str, u32)],
        outputs: &'static [(&'static str, u32)],
    },
    Knowledge {
        npc: &'static str,
        id: &'static str,
    },
    Memory {
        npc: &'static str,
        id: &'static str,
    },
    Storage {
        storage: &'static str,
        item: &'static str,
        count: u32,
    },
}
impl ColdEvent {
    fn event(self) -> forge_kernel::Event {
        use forge_kernel::{Event, EventKind};
        let kind = match self.kind {
            ColdEventKind::Mechanical { value } => return staff_event(self.turn, value).event(),
            ColdEventKind::Recipe {
                recipe,
                inputs,
                outputs,
            } => EventKind::RecipeApplied {
                recipe: recipe.into(),
                inputs: inputs.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
                outputs: outputs.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            },
            ColdEventKind::Knowledge { npc, id } => EventKind::NpcKnowledgeAdded {
                npc: npc.into(),
                knowledge: id.into(),
            },
            ColdEventKind::Memory { npc, id } => EventKind::NpcMemoryAdded {
                npc: npc.into(),
                memory: id.into(),
            },
            ColdEventKind::Storage {
                storage,
                item,
                count,
            } => EventKind::StorageItemTransferredToCharacter {
                storage: storage.into(),
                item: item.into(),
                count,
            },
        };
        Event {
            turn: self.turn,
            kind,
        }
    }
}
fn cold_selected_event(event: &forge_kernel::Event) -> bool {
    use forge_kernel::EventKind;
    staffing_mechanical_event(event)
        || matches!(
            event.kind,
            EventKind::RecipeApplied { .. }
                | EventKind::NpcMemoryAdded { .. }
                | EventKind::NpcKnowledgeAdded { .. }
                | EventKind::StorageItemTransferredToCharacter { .. }
                | EventKind::CharacterItemTransferredToStorage { .. }
        )
}
fn validate_cold_spec(history: &ColdShiftHistory, spec: &ScenarioSpec) -> Result<(), VerifyError> {
    let end = spec
        .expectations
        .final_world_time
        .ok_or_else(|| VerifyError::new("cold history needs final time"))?;
    let mut prior = 0;
    for step in history.steps {
        if step.index <= prior
            || step.index > spec.steps.len()
            || spec.steps[step.index - 1].definition_id != step.definition
            || step.before >= step.after
            || step.after > end
            || step
                .events
                .iter()
                .any(|e| e.turn < step.before || e.turn > step.after)
        {
            return Err(VerifyError::new("cold history checkpoint binding differs"));
        }
        prior = step.index;
    }
    if history.steps.is_empty()
        || (end >= 16) != history.surge.is_some()
        || history.surge.is_some_and(|t| t < 16 || t > end)
    {
        return Err(VerifyError::new("cold history deadline differs"));
    }
    let mut ids = BTreeSet::new();
    for r in history.records {
        if r.turn > end
            || !r.id.starts_with("fume_yards.")
            || r.subject.is_empty()
            || !ids.insert((r.npc, r.id, r.memory))
        {
            return Err(VerifyError::new("cold record binding differs"));
        }
    }
    if history.stocks.len() != 4
        || history
            .stocks
            .iter()
            .map(|s| s.npc)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([C_NESSA, C_BRANN, C_DARO, C_PERA])
    {
        return Err(VerifyError::new("cold stock owner binding differs"));
    }
    Ok(())
}
fn validate_cold_history(
    history: &ColdShiftHistory,
    trace: &forge_replay::Trace,
    state: &forge_kernel::GameState,
) -> Result<(), VerifyError> {
    for expected in history.steps {
        let index = expected
            .index
            .checked_sub(1)
            .ok_or_else(|| VerifyError::new("cold index zero"))?;
        let step = trace
            .steps
            .get(index)
            .ok_or_else(|| VerifyError::new("cold checkpoint missing"))?;
        let before = if index == 0 {
            trace.initial_state.world.time
        } else {
            trace.steps[index - 1].observation.world_time
        };
        if step.action.definition_id != expected.definition
            || before != expected.before
            || step.observation.world_time != expected.after
        {
            return Err(VerifyError::new("cold checkpoint time/action differs"));
        }
        let actual: Vec<_> = step
            .events
            .iter()
            .filter(|e| cold_selected_event(e))
            .cloned()
            .collect();
        let expected: Vec<_> = expected
            .events
            .iter()
            .copied()
            .map(ColdEvent::event)
            .collect();
        if actual != expected {
            return Err(VerifyError::new(
                "cold recipe, source, payment or movement event order differs",
            ));
        }
    }
    let count = state
        .world
        .npcs
        .values()
        .map(|n| {
            n.memories
                .keys()
                .chain(n.knowledge.keys())
                .filter(|id| id.starts_with("fume_yards."))
                .count()
        })
        .sum::<usize>();
    if count != history.records.len() {
        return Err(VerifyError::new("cold record inventory differs"));
    }
    for expected in history.records {
        let npc = state
            .world
            .npcs
            .get(expected.npc)
            .ok_or_else(|| VerifyError::new("cold record owner missing"))?;
        let actual = if expected.memory {
            npc.memories
                .get(expected.id)
                .map(|r| (&r.subject, &r.provenance, r.turn))
        } else {
            npc.knowledge
                .get(expected.id)
                .map(|r| (&r.subject, &r.provenance, r.turn))
        };
        if actual.is_none_or(|(subject, provenance, turn)| {
            subject != expected.subject
                || turn != expected.turn
                || !knowledge_provenance_matches(&expected.provenance, provenance)
        }) {
            return Err(VerifyError::new(format!(
                "cold record subject/source/time differs: {} {}",
                expected.npc, expected.id
            )));
        }
    }
    let player: BTreeMap<String, u32> = history
        .player_items
        .iter()
        .map(|(k, v)| ((*k).into(), *v))
        .collect();
    if state.character.inventory != player {
        return Err(VerifyError::new("cold exact player inventory differs"));
    }
    for stock in history.stocks {
        let expected: BTreeMap<String, u32> =
            stock.items.iter().map(|(k, v)| ((*k).into(), *v)).collect();
        if state
            .world
            .npcs
            .get(stock.npc)
            .is_none_or(|npc| npc.inventory != expected)
        {
            return Err(VerifyError::new("cold exact NPC stock differs"));
        }
    }
    let actual:Vec<_>=state.event_log.iter().filter(|e|matches!(&e.kind,forge_kernel::EventKind::ScheduledEventResolved{event_id,..} if event_id=="lowsail.next_surge")).cloned().collect();
    let expected: Vec<_> = history
        .surge
        .map(|turn| staff_event(turn, StaffingEventKind::Surge { applied: false }).event())
        .into_iter()
        .collect();
    if actual != expected {
        return Err(VerifyError::new("cold absolute surge history differs"));
    }
    let pending: Vec<_> = state
        .world
        .scheduled_events
        .iter()
        .filter(|e| e.id == "lowsail.next_surge")
        .collect();
    if pending.len() != usize::from(history.surge.is_none())
        || pending
            .iter()
            .any(|e| e.due_time != 16 || e.event_kind != "deadline")
    {
        return Err(VerifyError::new("cold absolute pending queue differs"));
    }
    Ok(())
}
const C_W: &str = "fume_yards.workshop";
const C_K: &str = "fume_yards.kiln_bay";
const C_A: &str = "fume_yards.ash_beds";
const C_L: &str = "lowsail.return";
const C_NESSA: &str = "fume_yards.nessa_tern";
const C_BRANN: &str = "fume_yards.brann_coil";
const C_DARO: &str = "fume_yards.daro_venn";
const C_PERA: &str = "fume_yards.pera_senn";
const C_TEST: &str = "fume_yards.charge_dust_test";
const C_CHOICES: &[(&str, &str)] = &[
    ("lineage", "fenborn"),
    ("origin", "lowsail"),
    ("calling", "ledger-clerk"),
    ("value", "order"),
    ("burden", "wanted"),
    ("history", "stole-permit"),
];
const C_START: ScenarioStartSpec = ScenarioStartSpec::Custom {
    name: "Cold shift comparison",
    choices: C_CHOICES,
};
const C_P12: [ScenarioStep; 12] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("fume_yards.take_stock"),
        action!("travel_adjacent","destination"=>"fume_yards.kiln_bay"),
        action!("fume_yards.prepare_charge"),
        action!("travel_adjacent","destination"=>"fume_yards.workshop"),
    ],
);
const C_T13: [ScenarioStep; 13] =
    append_steps(&C_P12, &[action!("fume_yards.test_unfired_charge")]);
const C_R14: [ScenarioStep; 14] = append_steps(&C_T13, &[action!("fume_yards.report_test")]);
const C_U14: [ScenarioStep; 14] = append_steps(
    &C_T13,
    &[action!("travel_adjacent","destination"=>"fume_yards.kiln_bay")],
);
const C_N14: [ScenarioStep; 14] = append_steps(
    &C_P12,
    &[
        action!("wait_tide"),
        action!("travel_adjacent","destination"=>"fume_yards.kiln_bay"),
    ],
);
const C_D17: [ScenarioStep; 15] =
    append_steps(&C_R14, &[action!("fume_yards.delegate_cold_shift")]);
const C_P17: [ScenarioStep; 17] = append_steps(
    &C_R14,
    &[
        action!("fume_yards.reclaim_charge"),
        action!("fume_yards.load_kiln_freight"),
        action!("fume_yards.return_with_nessa"),
    ],
);
const C_NW15: [ScenarioStep; 15] = append_steps(
    &C_R14,
    &[action!("travel_adjacent","destination"=>"fume_yards.workshop")],
);
const C_NR15: [ScenarioStep; 15] = append_steps(&C_R14, &[action!("fume_yards.return_with_nessa")]);
const C_BR18: [ScenarioStep; 16] =
    append_steps(&C_D17, &[action!("fume_yards.return_brann_to_kiln")]);
const C_BW21: [ScenarioStep; 19] = append_steps(
    &C_D17,
    &[
        action!("travel_adjacent","destination"=>"fume_yards.kiln_bay"),
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
    ],
);
const C_RP22: [ScenarioStep; 20] = append_steps(
    &C_BR18,
    &[
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
        action!("fume_yards.report_with_daro"),
    ],
);
const C_WATER: &[ScenarioStep] = &[
    action!("world.enter_aftermath"),
    action!("return.patch_stand"),
    action!("return.order_water_stand"),
    action!("return.visit_workshop"),
    action!("travel_adjacent","destination"=>"fume_yards.ash_beds"),
    action!("fume_yards.buy_collateral_filter"),
    action!("travel_adjacent","destination"=>"fume_yards.workshop"),
    action!("travel_adjacent","destination"=>"fume_yards.kiln_bay"),
    action!("fume_yards.take_market_cask"),
    action!("fume_yards.escort_market_cask"),
    action!("return.fit_market_filter"),
    action!("return.install_market_cask"),
    action!("return.draw_clean_water"),
];
const C_DW30: [ScenarioStep; 28] = append_steps(&C_D17, C_WATER);
const C_PW30: [ScenarioStep; 30] = append_steps(&C_P17, C_WATER);
const C_PREP: ScenarioRecipeExpectation = batch_recipe(
    10,
    "fume_yards.prepare_charge",
    &[("fume_yards.clay", 2), ("fume_yards.mesh", 1)],
    &[("fume_yards.prepared_charge", 1)],
);
const C_RECLAIM: ScenarioRecipeExpectation = batch_recipe(
    14,
    "fume_yards.reclaim_charge",
    &[("fume_yards.prepared_charge", 1)],
    &[("fume_yards.repair_lot", 1)],
);
const C_REPAIR_RECIPES: &[ScenarioRecipeExpectation] = &[C_PREP, C_RECLAIM];
const C_WATER_RECIPES: &[ScenarioRecipeExpectation] = &[
    C_PREP,
    C_RECLAIM,
    batch_recipe(
        18,
        "fume_yards.patch_stand",
        &[("fume_yards.repair_lot", 1)],
        &[],
    ),
    batch_recipe(
        27,
        "fume_yards.fit_market_filter",
        &[("fume_yards.filter", 1)],
        &[],
    ),
    batch_recipe(
        28,
        "fume_yards.install_market_cask",
        &[("fume_yards.water_cask", 1)],
        &[],
    ),
];
const fn cm(npc: &'static str, id: &'static str, subject: &'static str, turn: u64) -> ColdRecord {
    ColdRecord {
        npc,
        id,
        subject,
        turn,
        memory: true,
        provenance: ScenarioKnowledgeProvenance::Witnessed,
    }
}
const fn ck(npc: &'static str, id: &'static str, subject: &'static str, turn: u64) -> ColdRecord {
    ColdRecord {
        memory: false,
        ..cm(npc, id, subject, turn)
    }
}
const fn ct(
    npc: &'static str,
    id: &'static str,
    subject: &'static str,
    turn: u64,
    by: &'static str,
    memory: bool,
) -> ColdRecord {
    ColdRecord {
        memory,
        provenance: ScenarioKnowledgeProvenance::Told { by },
        ..cm(npc, id, subject, turn)
    }
}
const C_STOCK_REC: ColdRecord = cm(
    C_NESSA,
    "fume_yards.stock_handed_over",
    "Nessa handed the player two clay and one mesh.",
    8,
);
const C_PREP_REC: ColdRecord = cm(
    C_BRANN,
    "fume_yards.charge_prepared",
    "Brann watched the player prepare one kiln charge.",
    10,
);
const C_TEST_K: ColdRecord = ck(
    C_NESSA,
    C_TEST,
    "Nessa's covered tray caught dust from the player's unfired charge.",
    12,
);
const C_TEST_M: ColdRecord = cm(
    C_NESSA,
    C_TEST,
    "Nessa's covered tray caught dust from the player's unfired charge.",
    12,
);
const C_REPORT_K: ColdRecord = ct(
    C_BRANN,
    C_TEST,
    "Nessa reported her covered-tray dust finding to Brann.",
    13,
    C_NESSA,
    false,
);
const C_REPORT_M: ColdRecord = ct(
    C_BRANN,
    "fume_yards.charge_dust_report",
    "Nessa reported her covered-tray dust finding to Brann.",
    13,
    C_NESSA,
    true,
);
const C_VISIT_REC: ColdRecord = cm(
    C_NESSA,
    "fume_yards.dust_test_visit",
    "Nessa brought her dust finding to Brann at Kiln Bay.",
    13,
);
const C_CREW_REC: ColdRecord = cm(
    C_BRANN,
    "fume_yards.crew_charge_reclaimed",
    "Brann reclaimed the player's unfired charge.",
    14,
);
const C_CREW_PAY: ColdRecord = cm(
    C_BRANN,
    "fume_yards.kiln_freight_paid",
    "Brann paid three coins for the player's commissioned cold loading.",
    14,
);
const C_CREW_DONE: ColdRecord = cm(
    C_BRANN,
    "fume_yards.cold_shift_completed",
    "Brann finished the cold shift and closed the kiln.",
    14,
);
const C_PERSONAL_REC: ColdRecord = cm(
    C_BRANN,
    "fume_yards.charge_reclaimed",
    "Brann watched the player reclaim the unfired charge.",
    14,
);
const C_PERSONAL_PAY: ColdRecord = cm(
    C_BRANN,
    "fume_yards.kiln_freight_paid",
    "Brann paid the player three coins for loading kiln freight.",
    15,
);
const fn c_nessa_return(turn: u64) -> ColdRecord {
    cm(
        C_NESSA,
        "fume_yards.returned_from_dust_test",
        "Nessa returned to Workshop with the player after reporting her test.",
        turn,
    )
}
const C_BRANN_RETURN: ColdRecord = cm(
    C_BRANN,
    "fume_yards.returned_from_cold_shift",
    "Brann returned to Kiln Bay with the player after cold work.",
    17,
);
const C_BRACE_REC: ColdRecord = cm(
    C_DARO,
    "fume_yards.rack_braced",
    "Daro watched the player brace his jammed rack.",
    19,
);
const C_RACK_K: ColdRecord = ck(
    C_DARO,
    "fume_yards.rack_cleared",
    "Daro watched the player clear his jammed freight rack.",
    20,
);
const C_RACK_M: ColdRecord = cm(
    C_DARO,
    "fume_yards.rack_cleared",
    "Daro paid salvage rights for clearing his freight rack.",
    20,
);
const C_RACK_REPORT: ColdRecord = ct(
    C_BRANN,
    "fume_yards.rack_cleared",
    "Daro reported the cleared freight rack to Brann.",
    21,
    C_DARO,
    false,
);
const C_RACK_PAY: ColdRecord = cm(
    C_BRANN,
    "fume_yards.rack_report_paid",
    "Brann paid one coin for the witnessed access-clearing report.",
    21,
);
const C_RACK_TOLD: ColdRecord = cm(
    C_DARO,
    "fume_yards.rack_reported",
    "Daro reported the cleared rack to Brann in person.",
    21,
);
const C_PATCH_K: ColdRecord = ck(
    "oren_pell",
    "fume_yards.stand_patched",
    "The player patched Oren's raised sorting stand.",
    18,
);
const C_PATCH_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.stand_patched",
    "Oren watched the player patch his sorting stand.",
    18,
);
const C_ORDER_K: ColdRecord = ck(
    "oren_pell",
    "fume_yards.market_water_ordered",
    "Oren ordered a filter and stored cask for his repaired stand.",
    19,
);
const C_ORDER_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.market_water_ordered",
    "Oren planned a finite water supply at his repaired stand.",
    19,
);
const C_BUY_K: ColdRecord = ck(
    C_DARO,
    "fume_yards.collateral_settled",
    "Daro witnessed settlement of his separate caged filter.",
    22,
);
const C_BUY_M: ColdRecord = cm(
    C_DARO,
    "fume_yards.collateral_paid",
    "Daro received four coins for his caged filter.",
    22,
);
const C_CASK_M: ColdRecord = cm(
    C_PERA,
    "fume_yards.market_cask_handed_over",
    "Pera handed the player her sole cask for Lowsail.",
    25,
);
const C_CASK_K: ColdRecord = ck(
    C_PERA,
    "fume_yards.market_cask",
    "Pera inspected the stored water cask loaded for Lowsail.",
    26,
);
const C_CASK_TOLD: ColdRecord = ct(
    "oren_pell",
    "fume_yards.market_cask",
    "Pera identified the delivered stored-water cask to Oren.",
    26,
    C_PERA,
    false,
);
const C_ESCORT_M: ColdRecord = cm(
    C_PERA,
    "fume_yards.market_cask_escorted",
    "Pera escorted the player's cask to Lowsail.",
    26,
);
const C_ARRIVE_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.market_cask_arrived",
    "Oren witnessed Pera arrive with the player and cask.",
    26,
);
const C_FILTER_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.market_filter_fitted",
    "Oren watched the player install one filter at his repaired stand.",
    27,
);
const C_INSTALL_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.market_cask_installed",
    "Oren watched the delivered cask become the stand's finite supply.",
    28,
);
const C_WATER_K: ColdRecord = ck(
    "oren_pell",
    "fume_yards.clean_water_supplied",
    "Oren witnessed one filtered ration supplied from the installed cask.",
    29,
);
const C_WATER_M: ColdRecord = cm(
    "oren_pell",
    "fume_yards.clean_water_supplied",
    "Oren supplied the player one clean-water ration.",
    29,
);
const fn ce(turn: u64, kind: ColdEventKind) -> ColdEvent {
    ColdEvent { turn, kind }
}
const fn cme(turn: u64, value: StaffingEventKind) -> ColdEvent {
    ce(turn, ColdEventKind::Mechanical { value })
}
const fn ctime(after: u64, ticks: u64) -> ColdEvent {
    cme(after - ticks, StaffingEventKind::Time { ticks })
}
const fn ccoin(turn: u64, amount: i64) -> ColdEvent {
    cme(
        turn,
        StaffingEventKind::Resource {
            resource: "coin",
            amount,
        },
    )
}
const fn cstamina(turn: u64, amount: i64) -> ColdEvent {
    cme(
        turn,
        StaffingEventKind::Resource {
            resource: "stamina",
            amount,
        },
    )
}
const fn cnpc(turn: u64, npc: &'static str, from: &'static str, to: &'static str) -> ColdEvent {
    cme(turn, StaffingEventKind::NpcMoved { npc, from, to })
}
const fn cplayer(turn: u64, from: &'static str, to: &'static str) -> ColdEvent {
    cme(turn, StaffingEventKind::PlayerMoved { from, to })
}
const fn crec(r: ColdRecord) -> ColdEvent {
    ce(
        r.turn,
        if r.memory {
            ColdEventKind::Memory {
                npc: r.npc,
                id: r.id,
            }
        } else {
            ColdEventKind::Knowledge {
                npc: r.npc,
                id: r.id,
            }
        },
    )
}
const fn crecipe(r: ScenarioRecipeExpectation) -> ColdEvent {
    ce(
        r.turn,
        ColdEventKind::Recipe {
            recipe: r.recipe,
            inputs: r.inputs,
            outputs: r.outputs,
        },
    )
}
const fn ctransfer(turn: u64, npc: &'static str, item: &'static str, count: u32) -> ColdEvent {
    cme(turn, StaffingEventKind::NpcTransfer { npc, item, count })
}
const C_STOCK_STEP: ColdStep = ColdStep {
    index: 9,
    definition: "fume_yards.take_stock",
    before: 8,
    after: 9,
    events: &[
        ctransfer(8, C_NESSA, "fume_yards.clay", 2),
        ctransfer(8, C_NESSA, "fume_yards.mesh", 1),
        crec(C_STOCK_REC),
        ctime(9, 1),
    ],
};
const C_PREP_STEP: ColdStep = ColdStep {
    index: 11,
    definition: "fume_yards.prepare_charge",
    before: 10,
    after: 11,
    events: &[crecipe(C_PREP), crec(C_PREP_REC), ctime(11, 1)],
};
const C_TEST_STEP: ColdStep = ColdStep {
    index: 13,
    definition: "fume_yards.test_unfired_charge",
    before: 12,
    after: 13,
    events: &[crec(C_TEST_K), crec(C_TEST_M), ctime(13, 1)],
};
const C_REPORT_STEP: ColdStep = ColdStep {
    index: 14,
    definition: "fume_yards.report_test",
    before: 13,
    after: 14,
    events: &[
        cnpc(13, C_NESSA, C_W, C_K),
        cplayer(13, C_W, C_K),
        crec(C_REPORT_K),
        crec(C_VISIT_REC),
        crec(C_REPORT_M),
        ctime(14, 1),
    ],
};
const C_DELEGATE_STEP: ColdStep = ColdStep {
    index: 15,
    definition: "fume_yards.delegate_cold_shift",
    before: 14,
    after: 17,
    events: &[
        crecipe(C_RECLAIM),
        ccoin(14, 3),
        crec(C_CREW_REC),
        crec(C_CREW_PAY),
        crec(C_CREW_DONE),
        cnpc(14, C_NESSA, C_K, C_W),
        cnpc(14, C_BRANN, C_K, C_W),
        cplayer(14, C_K, C_W),
        ctime(17, 3),
        cme(17, StaffingEventKind::Surge { applied: false }),
    ],
};
const C_PERSONAL_STEP: ColdStep = ColdStep {
    index: 15,
    definition: "fume_yards.reclaim_charge",
    before: 14,
    after: 15,
    events: &[crecipe(C_RECLAIM), crec(C_PERSONAL_REC), ctime(15, 1)],
};
const C_LOAD_STEP: ColdStep = ColdStep {
    index: 16,
    definition: "fume_yards.load_kiln_freight",
    before: 15,
    after: 16,
    events: &[
        cstamina(15, -2),
        ccoin(15, 3),
        crec(C_PERSONAL_PAY),
        ctime(16, 1),
        cme(16, StaffingEventKind::Surge { applied: false }),
    ],
};
const fn c_return_nessa(index: usize, turn: u64, events: &'static [ColdEvent]) -> ColdStep {
    ColdStep {
        index,
        definition: "fume_yards.return_with_nessa",
        before: turn,
        after: turn + 1,
        events,
    }
}
const C_PERSONAL_RETURN: ColdStep = c_return_nessa(
    17,
    16,
    &[
        cnpc(16, C_NESSA, C_K, C_W),
        cplayer(16, C_K, C_W),
        crec(c_nessa_return(16)),
        ctime(17, 1),
    ],
);
const C_CANCEL_STEP: ColdStep = c_return_nessa(
    15,
    14,
    &[
        cnpc(14, C_NESSA, C_K, C_W),
        cplayer(14, C_K, C_W),
        crec(c_nessa_return(14)),
        ctime(15, 1),
    ],
);
const C_BRANN_RETURN_STEP: ColdStep = ColdStep {
    index: 16,
    definition: "fume_yards.return_brann_to_kiln",
    before: 17,
    after: 18,
    events: &[
        cnpc(17, C_BRANN, C_W, C_K),
        cplayer(17, C_W, C_K),
        crec(C_BRANN_RETURN),
        ctime(18, 1),
    ],
};
const C_BRACE_STEP: ColdStep = ColdStep {
    index: 18,
    definition: "fume_yards.brace_rack",
    before: 19,
    after: 20,
    events: &[cstamina(19, -2), crec(C_BRACE_REC), ctime(20, 1)],
};
const C_RACK_STEP: ColdStep = ColdStep {
    index: 19,
    definition: "fume_yards.recover_braced_filter",
    before: 20,
    after: 21,
    events: &[
        ctransfer(20, C_DARO, "fume_yards.filter", 1),
        crec(C_RACK_K),
        crec(C_RACK_M),
        ctime(21, 1),
    ],
};
const C_RACK_REPORT_STEP: ColdStep = ColdStep {
    index: 20,
    definition: "fume_yards.report_with_daro",
    before: 21,
    after: 22,
    events: &[
        cnpc(21, C_DARO, C_A, C_K),
        cplayer(21, C_A, C_K),
        crec(C_RACK_REPORT),
        ccoin(21, 1),
        crec(C_RACK_PAY),
        crec(C_RACK_TOLD),
        ctime(22, 1),
    ],
};

const C_DW_19: ColdStep = ColdStep {
    index: 17,
    definition: "return.patch_stand",
    before: 18,
    after: 19,
    events: &[
        crecipe(C_WATER_RECIPES[2]),
        crec(C_PATCH_K),
        crec(C_PATCH_M),
        ctime(19, 1),
    ],
};
const C_DW_20: ColdStep = ColdStep {
    index: 18,
    definition: "return.order_water_stand",
    before: 19,
    after: 20,
    events: &[crec(C_ORDER_K), crec(C_ORDER_M), ctime(20, 1)],
};
const C_DW_23: ColdStep = ColdStep {
    index: 21,
    definition: "fume_yards.buy_collateral_filter",
    before: 22,
    after: 23,
    events: &[
        ccoin(22, -4),
        ce(
            22,
            ColdEventKind::Storage {
                storage: "fume_yards.collateral_cage",
                item: "fume_yards.filter",
                count: 1,
            },
        ),
        crec(C_BUY_K),
        crec(C_BUY_M),
        ctime(23, 1),
    ],
};
const C_DW_26: ColdStep = ColdStep {
    index: 24,
    definition: "fume_yards.take_market_cask",
    before: 25,
    after: 26,
    events: &[
        ctransfer(25, C_PERA, "fume_yards.water_cask", 1),
        crec(C_CASK_M),
        ctime(26, 1),
    ],
};
const C_DW_27: ColdStep = ColdStep {
    index: 25,
    definition: "fume_yards.escort_market_cask",
    before: 26,
    after: 27,
    events: &[
        crec(C_CASK_K),
        cnpc(26, C_PERA, C_K, C_L),
        cplayer(26, C_K, C_L),
        crec(C_CASK_TOLD),
        crec(C_ESCORT_M),
        crec(C_ARRIVE_M),
        ctime(27, 1),
    ],
};
const C_DW_28: ColdStep = ColdStep {
    index: 26,
    definition: "return.fit_market_filter",
    before: 27,
    after: 28,
    events: &[crecipe(C_WATER_RECIPES[3]), crec(C_FILTER_M), ctime(28, 1)],
};
const C_DW_29: ColdStep = ColdStep {
    index: 27,
    definition: "return.install_market_cask",
    before: 28,
    after: 29,
    events: &[crecipe(C_WATER_RECIPES[4]), crec(C_INSTALL_M), ctime(29, 1)],
};
const C_DW_30: ColdStep = ColdStep {
    index: 28,
    definition: "return.draw_clean_water",
    before: 29,
    after: 30,
    events: &[
        cstamina(29, 2),
        crec(C_WATER_K),
        crec(C_WATER_M),
        ctime(30, 1),
    ],
};
const C_PW_19: ColdStep = ColdStep {
    index: 19,
    definition: "return.patch_stand",
    before: 18,
    after: 19,
    events: &[
        crecipe(C_WATER_RECIPES[2]),
        crec(C_PATCH_K),
        crec(C_PATCH_M),
        ctime(19, 1),
    ],
};
const C_PW_20: ColdStep = ColdStep {
    index: 20,
    definition: "return.order_water_stand",
    before: 19,
    after: 20,
    events: &[crec(C_ORDER_K), crec(C_ORDER_M), ctime(20, 1)],
};
const C_PW_23: ColdStep = ColdStep {
    index: 23,
    definition: "fume_yards.buy_collateral_filter",
    before: 22,
    after: 23,
    events: &[
        ccoin(22, -4),
        ce(
            22,
            ColdEventKind::Storage {
                storage: "fume_yards.collateral_cage",
                item: "fume_yards.filter",
                count: 1,
            },
        ),
        crec(C_BUY_K),
        crec(C_BUY_M),
        ctime(23, 1),
    ],
};
const C_PW_26: ColdStep = ColdStep {
    index: 26,
    definition: "fume_yards.take_market_cask",
    before: 25,
    after: 26,
    events: &[
        ctransfer(25, C_PERA, "fume_yards.water_cask", 1),
        crec(C_CASK_M),
        ctime(26, 1),
    ],
};
const C_PW_27: ColdStep = ColdStep {
    index: 27,
    definition: "fume_yards.escort_market_cask",
    before: 26,
    after: 27,
    events: &[
        crec(C_CASK_K),
        cnpc(26, C_PERA, C_K, C_L),
        cplayer(26, C_K, C_L),
        crec(C_CASK_TOLD),
        crec(C_ESCORT_M),
        crec(C_ARRIVE_M),
        ctime(27, 1),
    ],
};
const C_PW_28: ColdStep = ColdStep {
    index: 28,
    definition: "return.fit_market_filter",
    before: 27,
    after: 28,
    events: &[crecipe(C_WATER_RECIPES[3]), crec(C_FILTER_M), ctime(28, 1)],
};
const C_PW_29: ColdStep = ColdStep {
    index: 29,
    definition: "return.install_market_cask",
    before: 28,
    after: 29,
    events: &[crecipe(C_WATER_RECIPES[4]), crec(C_INSTALL_M), ctime(29, 1)],
};
const C_PW_30: ColdStep = ColdStep {
    index: 30,
    definition: "return.draw_clean_water",
    before: 29,
    after: 30,
    events: &[
        cstamina(29, 2),
        crec(C_WATER_K),
        crec(C_WATER_M),
        ctime(30, 1),
    ],
};
const C_DUST_TESTED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[C_STOCK_STEP, C_PREP_STEP, C_TEST_STEP],
    records: &[C_STOCK_REC, C_PREP_REC, C_TEST_K, C_TEST_M],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_TESTED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-tested",
    claim_id: "milestone-2.cold-shift.dust-tested",
    start: C_START,
    seed: 71,
    steps: &C_T13,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_TESTED_HISTORY),
        final_location: C_W,
        final_action_definition: C_T13[C_T13.len() - 1].definition_id,
        final_observation_contains: "Bring Nessa and your charge to Kiln Bay",
        forbidden_observation_contains: &[],
        final_world_time: Some(13),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.report_test", "fume_yards.load_freight"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
        ],
    },
};
const C_DUST_RELAYED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[C_STOCK_STEP, C_PREP_STEP, C_TEST_STEP, C_REPORT_STEP],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
    ],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_RELAYED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-relayed",
    claim_id: "milestone-2.cold-shift.dust-relayed",
    start: C_START,
    seed: 71,
    steps: &C_R14,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_RELAYED_HISTORY),
        final_location: C_K,
        final_action_definition: C_R14[C_R14.len() - 1].definition_id,
        final_observation_contains: "Your council ink and wanted face trouble Brann, but Nessa's report earns his help.",
        forbidden_observation_contains: &[],
        final_world_time: Some(14),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_K),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &[
            "fume_yards.delegate_cold_shift",
            "fume_yards.reclaim_charge",
        ],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.report_test",
        ],
    },
};
const C_DUST_UNRELAYED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[C_STOCK_STEP, C_PREP_STEP, C_TEST_STEP],
    records: &[C_STOCK_REC, C_PREP_REC, C_TEST_K, C_TEST_M],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_UNRELAYED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-unrelayed",
    claim_id: "milestone-2.cold-shift.dust-unrelayed",
    start: C_START,
    seed: 71,
    steps: &C_U14,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_UNRELAYED_HISTORY),
        final_location: C_K,
        final_action_definition: C_U14[C_U14.len() - 1].definition_id,
        final_observation_contains: "Ignite spends charge and fuel",
        forbidden_observation_contains: &[],
        final_world_time: Some(14),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.reclaim_charge"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
        ],
    },
};
const C_DUST_NO_TEST_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[C_STOCK_STEP, C_PREP_STEP],
    records: &[C_STOCK_REC, C_PREP_REC],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_NO_TEST_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-no-test",
    claim_id: "milestone-2.cold-shift.dust-no-test",
    start: C_START,
    seed: 71,
    steps: &C_N14,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_NO_TEST_HISTORY),
        final_location: C_K,
        final_action_definition: C_N14[C_N14.len() - 1].definition_id,
        final_observation_contains: "Ignite spends charge and fuel",
        forbidden_observation_contains: &[],
        final_world_time: Some(14),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.reclaim_charge"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
        ],
    },
};
const C_COLD_DELEGATED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_DELEGATE_STEP,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_CREW_REC,
        C_CREW_PAY,
        C_CREW_DONE,
    ],
    player_items: &[("rope", 1), ("fume_yards.repair_lot", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: Some(17),
};
const COLD_COLD_DELEGATED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-delegated",
    claim_id: "milestone-2.cold-shift.cold-delegated",
    start: C_START,
    seed: 71,
    steps: &C_D17,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_DELEGATED_HISTORY),
        final_location: C_W,
        final_action_definition: C_D17[C_D17.len() - 1].definition_id,
        final_observation_contains: "Return Brann to Kiln Bay after his paid shift",
        forbidden_observation_contains: &[],
        final_world_time: Some(17),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_W),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.repair_lot", 1),
        ],
        required_character_resources: &[market_resource("coin", 13), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_REPAIR_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.return_brann_to_kiln", "fume_yards.load_freight"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
        ],
    },
};
const C_COLD_PERSONAL_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_PERSONAL_STEP,
        C_LOAD_STEP,
        C_PERSONAL_RETURN,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_PERSONAL_REC,
        C_PERSONAL_PAY,
        c_nessa_return(16),
    ],
    player_items: &[("rope", 1), ("fume_yards.repair_lot", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: Some(16),
};
const COLD_COLD_PERSONAL_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-personal",
    claim_id: "milestone-2.cold-shift.cold-personal",
    start: C_START,
    seed: 71,
    steps: &C_P17,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_PERSONAL_HISTORY),
        final_location: C_W,
        final_action_definition: C_P17[C_P17.len() - 1].definition_id,
        final_observation_contains: "Nessa returns with you to Workshop",
        forbidden_observation_contains: &[],
        final_world_time: Some(17),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.repair_lot", 1),
        ],
        required_character_resources: &[market_resource("coin", 13), market_resource("stamina", 1)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_REPAIR_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["wait_tide"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.load_freight",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
        ],
    },
};
const C_DUST_NESSA_WALKED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[C_STOCK_STEP, C_PREP_STEP, C_TEST_STEP, C_REPORT_STEP],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
    ],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_NESSA_WALKED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-nessa-walked",
    claim_id: "milestone-2.cold-shift.dust-nessa-walked",
    start: C_START,
    seed: 71,
    steps: &C_NW15,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_NESSA_WALKED_HISTORY),
        final_location: C_W,
        final_action_definition: C_NW15[C_NW15.len() - 1].definition_id,
        final_observation_contains: "Nessa remains at Kiln Bay",
        forbidden_observation_contains: &[],
        final_world_time: Some(15),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_K),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["wait_tide"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.load_freight",
        ],
    },
};
const C_DUST_NESSA_RETURNED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_CANCEL_STEP,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        c_nessa_return(14),
    ],
    player_items: &[("rope", 1), ("fume_yards.prepared_charge", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: None,
};
const COLD_DUST_NESSA_RETURNED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-dust-nessa-returned",
    claim_id: "milestone-2.cold-shift.dust-nessa-returned",
    start: C_START,
    seed: 71,
    steps: &C_NR15,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_DUST_NESSA_RETURNED_HISTORY),
        final_location: C_W,
        final_action_definition: C_NR15[C_NR15.len() - 1].definition_id,
        final_observation_contains: "Brann retains Nessa's report at Kiln Bay",
        forbidden_observation_contains: &[],
        final_world_time: Some(15),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.prepared_charge", 1),
        ],
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: &[C_PREP],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.load_freight"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
        ],
    },
};
const C_COLD_BRANN_WALKED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_DELEGATE_STEP,
        C_BRACE_STEP,
        C_RACK_STEP,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_CREW_REC,
        C_CREW_PAY,
        C_CREW_DONE,
        C_BRACE_REC,
        C_RACK_K,
        C_RACK_M,
    ],
    player_items: &[
        ("rope", 1),
        ("fume_yards.repair_lot", 1),
        ("fume_yards.filter", 1),
    ],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: Some(17),
};
const COLD_COLD_BRANN_WALKED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-brann-walked",
    claim_id: "milestone-2.cold-shift.cold-brann-walked",
    start: C_START,
    seed: 71,
    steps: &C_BW21,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_BRANN_WALKED_HISTORY),
        final_location: C_A,
        final_action_definition: C_BW21[C_BW21.len() - 1].definition_id,
        final_observation_contains: "Return Brann from Workshop to Kiln Bay",
        forbidden_observation_contains: &[],
        final_world_time: Some(21),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_W),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.repair_lot", 1),
            market_item("fume_yards.filter", 1),
        ],
        required_character_resources: &[market_resource("coin", 13), market_resource("stamina", 1)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_REPAIR_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.leave_ash_hatch"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
            "fume_yards.recover_braced_filter",
            "fume_yards.report_with_daro",
        ],
    },
};
const C_COLD_BRANN_RETURNED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_DELEGATE_STEP,
        C_BRANN_RETURN_STEP,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_CREW_REC,
        C_CREW_PAY,
        C_CREW_DONE,
        C_BRANN_RETURN,
    ],
    player_items: &[("rope", 1), ("fume_yards.repair_lot", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: Some(17),
};
const COLD_COLD_BRANN_RETURNED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-brann-returned",
    claim_id: "milestone-2.cold-shift.cold-brann-returned",
    start: C_START,
    seed: 71,
    steps: &C_BR18,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_BRANN_RETURNED_HISTORY),
        final_location: C_K,
        final_action_definition: C_BR18[C_BR18.len() - 1].definition_id,
        final_observation_contains: "Brann returns with you to Kiln Bay",
        forbidden_observation_contains: &[],
        final_world_time: Some(18),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.repair_lot", 1),
        ],
        required_character_resources: &[market_resource("coin", 13), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_REPAIR_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.enter_ash_hatch"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
        ],
    },
};
const C_COLD_RACK_PAID_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_DELEGATE_STEP,
        C_BRANN_RETURN_STEP,
        C_BRACE_STEP,
        C_RACK_STEP,
        C_RACK_REPORT_STEP,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_CREW_REC,
        C_CREW_PAY,
        C_CREW_DONE,
        C_BRACE_REC,
        C_RACK_K,
        C_RACK_M,
        C_BRANN_RETURN,
        C_RACK_REPORT,
        C_RACK_PAY,
        C_RACK_TOLD,
    ],
    player_items: &[
        ("rope", 1),
        ("fume_yards.repair_lot", 1),
        ("fume_yards.filter", 1),
    ],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[],
        },
        ColdStock {
            npc: C_PERA,
            items: &[("fume_yards.water_cask", 1)],
        },
    ],
    surge: Some(17),
};
const COLD_COLD_RACK_PAID_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-rack-paid",
    claim_id: "milestone-2.cold-shift.cold-rack-paid",
    start: C_START,
    seed: 71,
    steps: &C_RP22,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_RACK_PAID_HISTORY),
        final_location: C_K,
        final_action_definition: C_RP22[C_RP22.len() - 1].definition_id,
        final_observation_contains: "Brann pays one coin",
        forbidden_observation_contains: &[],
        final_world_time: Some(22),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_K),
                market_position(C_PERA, C_K),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.repair_lot", 1),
            market_item("fume_yards.filter", 1),
        ],
        required_character_resources: &[market_resource("coin", 14), market_resource("stamina", 1)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_REPAIR_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["fume_yards.enter_ash_hatch"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
            "fume_yards.recover_braced_filter",
            "fume_yards.report_with_daro",
        ],
    },
};
const C_COLD_WATER_DELEGATED_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_DELEGATE_STEP,
        C_DW_19,
        C_DW_20,
        C_DW_23,
        C_DW_26,
        C_DW_27,
        C_DW_28,
        C_DW_29,
        C_DW_30,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_CREW_REC,
        C_CREW_PAY,
        C_CREW_DONE,
        C_PATCH_K,
        C_PATCH_M,
        C_ORDER_K,
        C_ORDER_M,
        C_BUY_K,
        C_BUY_M,
        C_CASK_M,
        C_CASK_K,
        C_CASK_TOLD,
        C_ESCORT_M,
        C_ARRIVE_M,
        C_FILTER_M,
        C_INSTALL_M,
        C_WATER_K,
        C_WATER_M,
    ],
    player_items: &[("rope", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[],
        },
    ],
    surge: Some(17),
};
const COLD_COLD_WATER_DELEGATED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-water-delegated",
    claim_id: "milestone-2.cold-shift.cold-water-delegated",
    start: C_START,
    seed: 71,
    steps: &C_DW30,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_WATER_DELEGATED_HISTORY),
        final_location: C_L,
        final_action_definition: C_DW30[C_DW30.len() - 1].definition_id,
        final_observation_contains: "Oren supplies one clean-water ration",
        forbidden_observation_contains: &[],
        final_world_time: Some(30),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_W),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_L),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[market_item("rope", 1)],
        required_character_resources: &[market_resource("coin", 9), market_resource("stamina", 5)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_WATER_RECIPES,
        storage_balances: MARKET_EMPTY_CAGE,
        storage_transfers: &[StorageTransferExpectation {
            turn: 22,
            direction: StorageTransferDirection::ToCharacter,
            storage: "fume_yards.collateral_cage",
            item: "fume_yards.filter",
            count: 1,
        }],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["return.sort_dry_goods"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
            "return.draw_clean_water",
            "return.fit_market_filter",
            "return.install_market_cask",
        ],
    },
};
const C_COLD_WATER_PERSONAL_HISTORY: ColdShiftHistory = ColdShiftHistory {
    steps: &[
        C_STOCK_STEP,
        C_PREP_STEP,
        C_TEST_STEP,
        C_REPORT_STEP,
        C_PERSONAL_STEP,
        C_LOAD_STEP,
        C_PERSONAL_RETURN,
        C_PW_19,
        C_PW_20,
        C_PW_23,
        C_PW_26,
        C_PW_27,
        C_PW_28,
        C_PW_29,
        C_PW_30,
    ],
    records: &[
        C_STOCK_REC,
        C_PREP_REC,
        C_TEST_K,
        C_TEST_M,
        C_REPORT_K,
        C_REPORT_M,
        C_VISIT_REC,
        C_PERSONAL_REC,
        C_PERSONAL_PAY,
        c_nessa_return(16),
        C_PATCH_K,
        C_PATCH_M,
        C_ORDER_K,
        C_ORDER_M,
        C_BUY_K,
        C_BUY_M,
        C_CASK_M,
        C_CASK_K,
        C_CASK_TOLD,
        C_ESCORT_M,
        C_ARRIVE_M,
        C_FILTER_M,
        C_INSTALL_M,
        C_WATER_K,
        C_WATER_M,
    ],
    player_items: &[("rope", 1)],
    stocks: &[
        ColdStock {
            npc: C_NESSA,
            items: &[],
        },
        ColdStock {
            npc: C_BRANN,
            items: &[("fume_yards.fuel", 1)],
        },
        ColdStock {
            npc: C_DARO,
            items: &[("fume_yards.filter", 1)],
        },
        ColdStock {
            npc: C_PERA,
            items: &[],
        },
    ],
    surge: Some(16),
};
const COLD_COLD_WATER_PERSONAL_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-cold-water-personal",
    claim_id: "milestone-2.cold-shift.cold-water-personal",
    start: C_START,
    seed: 71,
    steps: &C_PW30,
    expectations: ScenarioExpectations {
        staffing_history: None,
        cold_shift_history: Some(&C_COLD_WATER_PERSONAL_HISTORY),
        final_location: C_L,
        final_action_definition: C_PW30[C_PW30.len() - 1].definition_id,
        final_observation_contains: "Oren supplies one clean-water ration",
        forbidden_observation_contains: &[],
        final_world_time: Some(30),
        exclusive_after_action: "",
        required_world_flags: &[
            "ending_council",
            "flow_locked_market",
            "sluice_outcome_chosen",
        ],
        forbidden_world_flags: &["surge_missed", "sluice_failure"],
        required_location_flags: &[
            (C_A, "fume_yards.collateral_settled"),
            (C_K, "fume_yards.cask_taken"),
            (C_K, "fume_yards.charge_prepared"),
            (C_K, "fume_yards.kiln_closed"),
            (C_K, "fume_yards.kiln_freight_loaded"),
            (C_K, "fume_yards.kiln_reclaimed"),
            (C_K, "fume_yards.test_report_delivered"),
            (C_L, "fume_yards.clean_water_drawn"),
            (C_L, "fume_yards.market_cask_delivered"),
            (C_L, "fume_yards.market_cask_installed"),
            (C_L, "fume_yards.market_filter_fitted"),
            (C_L, "fume_yards.market_water_ordered"),
            (C_L, "fume_yards.stand_patched"),
            (C_W, "fume_yards.stock_given"),
        ],
        forbidden_location_flags: &[
            (C_A, "fume_yards.rack_braced"),
            (C_A, "fume_yards.rack_cleared"),
            (C_A, "fume_yards.rack_rear_open"),
            (C_A, "fume_yards.report_paid"),
            (C_A, "fume_yards.salvage_assignment_active"),
            (C_A, "fume_yards.salvage_assignment_spent"),
            (C_A, "fume_yards.staffed_rack_lift_completed"),
            (C_K, "fume_yards.batch_active"),
            (C_K, "fume_yards.batch_drawn"),
            (C_K, "fume_yards.batch_ignited"),
            (C_K, "fume_yards.batch_spoiled"),
            (C_K, "fume_yards.cold_shift_completed"),
            (C_K, "fume_yards.cold_work_chosen"),
            (C_K, "fume_yards.dust_filter_fitted"),
            (C_K, "fume_yards.freight_spoiled"),
            (C_K, "fume_yards.fuel_settled"),
            (C_K, "fume_yards.fuel_taken"),
            (C_K, "fume_yards.kiln_banked"),
            (C_K, "fume_yards.wet_screen_fitted"),
            (C_L, "fume_yards.dry_goods_sorted"),
            (C_L, "fume_yards.filter_sold"),
            (C_W, "fume_yards.freight_loaded"),
            (C_W, "fume_yards.screen_fitted"),
        ],
        required_deeds: &["stole_permit", "accepted_council"],
        required_visited_locations: &[C_W, C_K],
        required_npc_locations: &market_append::<_, 9>(
            AFTERMATH_NPC_LOCATIONS,
            &[
                market_position(C_NESSA, C_W),
                market_position(C_BRANN, C_K),
                market_position(C_DARO, C_A),
                market_position(C_PERA, C_L),
            ],
        ),
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: &[],
        required_npc_memories: &[],
        required_character_inventory: &[market_item("rope", 1)],
        required_character_resources: &[market_resource("coin", 9), market_resource("stamina", 3)],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[],
        forbidden_character_inventory: &[],
        recipe_events: C_WATER_RECIPES,
        storage_balances: MARKET_EMPTY_CAGE,
        storage_transfers: &[StorageTransferExpectation {
            turn: 22,
            direction: StorageTransferDirection::ToCharacter,
            storage: "fume_yards.collateral_cage",
            item: "fume_yards.filter",
            count: 1,
        }],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["return.sort_dry_goods"],
        forbidden_legal_definitions: &[
            "fume_yards.assign_brann_salvage",
            "fume_yards.recover_staffed_filter",
            "fume_yards.return_with_brann",
            "fume_yards.ignite_batch",
            "fume_yards.delegate_cold_shift",
            "fume_yards.report_test",
            "fume_yards.reclaim_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.take_fuel",
            "fume_yards.fit_dust_filter",
            "return.draw_clean_water",
            "return.fit_market_filter",
            "return.install_market_cask",
        ],
    },
};
#[cfg(test)]
mod cold_shift_tests {
    use super::*;
    use forge_kernel::{EventKind, KnowledgeProvenance};
    const SOURCE: &str = include_str!("../../../content/split-tide.json");
    fn specs() -> Vec<&'static ScenarioSpec> {
        all()
            .iter()
            .filter(|s| s.expectations.cold_shift_history.is_some())
            .collect()
    }
    #[test]
    fn cold_shift_thirteen_literal_production_paths_preserve_sources_stock_and_time() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let specs = specs();
        assert_eq!(specs.len(), 13);
        assert_eq!(specs.iter().map(|s| s.steps.len()).sum::<usize>(), 230);
        validate_registry().unwrap();
        for spec in specs {
            run(spec, &content).unwrap_or_else(|e| panic!("{}: {e}", spec.id));
        }
        let delegated = run(&COLD_COLD_DELEGATED_SPEC, &content).unwrap();
        let personal = run(&COLD_COLD_PERSONAL_SPEC, &content).unwrap();
        assert_eq!(delegated.state().world.time, personal.state().world.time);
        assert_eq!(
            delegated.state().character.inventory,
            personal.state().character.inventory
        );
        assert_eq!(
            delegated.state().character.resources["stamina"]
                - personal.state().character.resources["stamina"],
            2
        );
    }
    #[test]
    fn cold_shift_oracle_rejects_wrong_event_order_deadline_and_physical_moves() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let session = run(&COLD_COLD_DELEGATED_SPEC, &content).unwrap();
        let history = COLD_COLD_DELEGATED_SPEC
            .expectations
            .cold_shift_history
            .unwrap();
        for fault in [
            "recipe",
            "payment",
            "movement",
            "order",
            "time",
            "due",
            "duplicate",
        ] {
            let mut trace = session.trace().clone();
            let events = &mut trace.steps[14].events;
            match fault {
                "recipe" => events.retain(|e| !matches!(e.kind, EventKind::RecipeApplied { .. })),
                "payment" => {
                    events
                        .iter_mut()
                        .find(|e| matches!(e.kind, EventKind::ResourceAdjusted { .. }))
                        .unwrap()
                        .kind = EventKind::ResourceAdjusted {
                        resource: "coin".into(),
                        amount: 4,
                    }
                }
                "movement" => events
                    .retain(|e| !matches!(&e.kind,EventKind::NpcMoved{npc,..} if npc==C_BRANN)),
                "order" => events.reverse(),
                "time" => {
                    events
                        .iter_mut()
                        .find(|e| matches!(e.kind, EventKind::TimeAdvanced { .. }))
                        .unwrap()
                        .kind = EventKind::TimeAdvanced { ticks: 1 }
                }
                "due" => {
                    events
                        .iter_mut()
                        .find(|e| matches!(e.kind, EventKind::ScheduledEventResolved { .. }))
                        .unwrap()
                        .turn = 16
                }
                "duplicate" => events.push(events[0].clone()),
                _ => unreachable!(),
            }
            assert!(
                validate_cold_history(history, &trace, session.state()).is_err(),
                "accepted {fault}"
            );
        }
        let mut state = session.state().clone();
        state
            .world
            .scheduled_events
            .push(forge_kernel::ScheduledEvent {
                id: "lowsail.next_surge".into(),
                due_time: 16,
                event_kind: "deadline".into(),
            });
        assert!(validate_cold_history(history, session.trace(), &state).is_err());
        let relayed = run(&COLD_DUST_RELAYED_SPEC, &content).unwrap();
        let mut state = relayed.state().clone();
        state.world.npcs.get_mut(C_NESSA).unwrap().location = C_W.into();
        assert!(
            validate_npc_locations(
                &state,
                COLD_DUST_RELAYED_SPEC.expectations.required_npc_locations
            )
            .is_err()
        );
    }
    #[test]
    fn cold_shift_oracle_rejects_invented_reports_subjects_stock_and_service_claims() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let session = run(&COLD_COLD_DELEGATED_SPEC, &content).unwrap();
        let history = COLD_COLD_DELEGATED_SPEC
            .expectations
            .cold_shift_history
            .unwrap();
        for fault in [
            "sender",
            "kind",
            "turn",
            "subject",
            "invented_witness",
            "charge",
            "fuel",
        ] {
            let mut state = session.state().clone();
            match fault {
                "sender" => {
                    state
                        .world
                        .npcs
                        .get_mut(C_BRANN)
                        .unwrap()
                        .knowledge
                        .get_mut(C_TEST)
                        .unwrap()
                        .provenance = KnowledgeProvenance::Told { by: C_DARO.into() }
                }
                "kind" => {
                    state
                        .world
                        .npcs
                        .get_mut(C_BRANN)
                        .unwrap()
                        .knowledge
                        .get_mut(C_TEST)
                        .unwrap()
                        .provenance = KnowledgeProvenance::Witnessed
                }
                "turn" => {
                    state
                        .world
                        .npcs
                        .get_mut(C_NESSA)
                        .unwrap()
                        .knowledge
                        .get_mut(C_TEST)
                        .unwrap()
                        .turn = 13
                }
                "subject" => {
                    state
                        .world
                        .npcs
                        .get_mut(C_BRANN)
                        .unwrap()
                        .memories
                        .get_mut("fume_yards.crew_charge_reclaimed")
                        .unwrap()
                        .subject = "Brann watched the player reclaim the unfired charge.".into()
                }
                "invented_witness" => {
                    let r = state.world.npcs[C_NESSA].knowledge[C_TEST].clone();
                    state
                        .world
                        .npcs
                        .get_mut(C_PERA)
                        .unwrap()
                        .knowledge
                        .insert(C_TEST.into(), r);
                }
                "charge" => {
                    state
                        .character
                        .inventory
                        .insert("fume_yards.prepared_charge".into(), 1);
                }
                "fuel" => state.world.npcs.get_mut(C_BRANN).unwrap().inventory.clear(),
                _ => unreachable!(),
            }
            assert!(
                validate_cold_history(history, session.trace(), &state).is_err(),
                "accepted {fault}"
            );
        }
        let unrelayed = run(&COLD_DUST_UNRELAYED_SPEC, &content).unwrap();
        let mut state = unrelayed.state().clone();
        state.world.npcs.get_mut(C_BRANN).unwrap().knowledge.insert(
            C_TEST.into(),
            session.state().world.npcs[C_BRANN].knowledge[C_TEST].clone(),
        );
        assert!(
            validate_cold_history(
                COLD_DUST_UNRELAYED_SPEC
                    .expectations
                    .cold_shift_history
                    .unwrap(),
                unrelayed.trace(),
                &state
            )
            .is_err()
        );
        let walked = run(&COLD_COLD_BRANN_WALKED_SPEC, &content).unwrap();
        let mut false_service = COLD_COLD_BRANN_WALKED_SPEC;
        false_service.expectations.required_legal_definitions = &["fume_yards.report_with_daro"];
        assert!(validate_session(&false_service, &walked, &content).is_err());
        let mut wrong = COLD_COLD_DELEGATED_SPEC;
        wrong.expectations.required_character_resources = &[ScenarioResourceExpectation {
            resource: "coin",
            amount: 14,
        }];
        assert!(validate_session(&wrong, &session, &content).is_err());
        let water = run(&COLD_COLD_WATER_DELEGATED_SPEC, &content).unwrap();
        let mut state = water.state().clone();
        state
            .world
            .npcs
            .get_mut(C_PERA)
            .unwrap()
            .inventory
            .insert("fume_yards.water_cask".into(), 1);
        assert!(
            validate_cold_history(
                COLD_COLD_WATER_DELEGATED_SPEC
                    .expectations
                    .cold_shift_history
                    .unwrap(),
                water.trace(),
                &state
            )
            .is_err()
        );
    }
}

#[cfg(test)]
#[test]
fn cold_shift_preserves_all_55_accepted_scenario_bindings() {
    // Literal accepted ca5b142 witness commitments, captured before cycle37 generation.
    let accepted = [
        (
            "m0-ilyan",
            "dd34d7dc8b59e9aa34f4108451a246294ed23d9c1195f0ad27550de8b7d29dd3",
        ),
        (
            "m0-rook",
            "e192fc82c2c5468e207172571059fa1afc26c87480ad1f60ca87043095618555",
        ),
        (
            "m1-area-lowsail-market",
            "5b8f8083a2056823b8a88215934f8c3df9d7f32012226473fc090bdc4c3958f2",
        ),
        (
            "m1-area-red-sluice",
            "0a621f5e9e825ec4d28214813d1b2480f0e75dcf81867eadff481e81af1361e0",
        ),
        (
            "m1-custom-cross-current",
            "f2445092b0d1bf9ffd32959fc38531a4e68d528f37b0462c08d447ded7a6ad9d",
        ),
        (
            "m1-custom-unlikely-ally",
            "20d0f2ba840d02ce9c0ed537851fbdd0bbae25fba6110d48605ef21c322bbe87",
        ),
        (
            "m1-deadline-missed-surge",
            "391ddf3ebb13ed3cab7ee3db75132875849c9120c8a388b34de1df9579b22cad",
        ),
        (
            "m1-outcome-break-toll",
            "70e4d4e819eea9fb62540816df2885955889243a4177598feb6c8c9593f577c0",
        ),
        (
            "m1-outcome-hold-market",
            "b351a252b5223219367b8290c0424707cd307db69c8a19f4a1c60998e1456d00",
        ),
        (
            "m1-outcome-overload-disaster",
            "fb7a2e7b47377156efce537ec03e3f3ee8a64d93f4a238792a84835ddb39a04b",
        ),
        (
            "m1-outcome-relief-channel",
            "6326eb33c28ed983194b539c918e2dc65212e5a3d860ce742ded579df687153f",
        ),
        (
            "m1-outcome-split-flow",
            "81cc732af6fa6dfd4bcc61cdde9d2c1445436fc69afd634f784be08019d189c2",
        ),
        (
            "m1-paid-towline-relief",
            "40454ffe797b96a17b4ccafa387a4e22e59cfd7403f65baf483eb0b5a502180e",
        ),
        (
            "m1-tide-key-split-flow",
            "99acf082a4c983506a6215a38ede7cfa4414df7a0edd2b01a47da8c1723faa53",
        ),
        (
            "m1-warning-relayed",
            "125f32030b2efc75e10194725f4151beceb83ba241dd488d5dfa30a4fa642c20",
        ),
        (
            "m1-warning-unrelayed",
            "65fb2678bb7f2647fe65f0d709144185448b271fc59df0b8d4ec4e13b949be30",
        ),
        (
            "m2-fume-bank-save-tide",
            "3ee0e67b73ce7b615e8953884c64b01a51fd2188dd28c141f0fd5d089fff3951",
        ),
        (
            "m2-fume-batch-ready",
            "83b7a39e6bef068a04fb5778efecf37cbf75de24a1e1a45b9f1a7b21aee810ac",
        ),
        (
            "m2-fume-cold-repair",
            "146b4bf92d52ce6297a29ef7e5c93c641fddd4fe21de43a38f6a9db9ff773b0c",
        ),
        (
            "m2-fume-cold-screen",
            "c27841aa9e5605ec8fc35c1b8d8de4bf59a70c6b7654995ae4304875715c2caa",
        ),
        (
            "m2-fume-collateral-after-report",
            "e8b5431d3c3892f6a619a4334213e8c8a802046809c5e8e22c3c2b667ac8ccfa",
        ),
        (
            "m2-fume-collateral-fuel",
            "a710398d4c864112a4672c7cc72ea39b9bee3acd0f798a2dfc3faaf3a04318a9",
        ),
        (
            "m2-fume-collateral-local",
            "bef42df4da9de5a79fd34a5b228276b41d728d882248e0ba84030c23817b19fa",
        ),
        (
            "m2-fume-collateral-purchase",
            "bcea4096952f01a12cc1e417843eb7e6719037ce542b226cc2e625e0e7ba9152",
        ),
        (
            "m2-fume-collateral-sale",
            "ccd4fa26344c8d77022b1641dd6fa240a093e1cc353734eda4155b7cbb7fcaf5",
        ),
        (
            "m2-fume-crew-cancelled",
            "4943b3f4daf76504076ae4c6bdba448d92ce0fb82bcaf8cc17ad7458994f30d0",
        ),
        (
            "m2-fume-crew-no-account",
            "7d6cc6821a69c1e101b16eee4a44e541f3e2fbf065593da9eedb254d2fa7631a",
        ),
        (
            "m2-fume-crew-no-help",
            "26faceb3ab9fa091351de860a52fc22bc354a54c29ccf9a3574e7bc7e041245e",
        ),
        (
            "m2-fume-crew-ordinary",
            "c993c5980c62defca297803f057ad2742a376cd8d00d1f08f540934414ec7e7c",
        ),
        (
            "m2-fume-crew-other-history",
            "c9e7a88703bbe42e08a2898259e483c83266bf52c4f98f76cfae1e86c9cfd0da",
        ),
        (
            "m2-fume-crew-staffed",
            "63fe3679a34d8d8ab802e2acf6e34dfcd75406d7be1b394e6f77dcfc7c0e4ef2",
        ),
        (
            "m2-fume-crew-walked-away",
            "f45417419cccf9f48d59d66e146cadd74e1b4a2291a882a503dcaaa80905d835",
        ),
        (
            "m2-fume-crew-water-composed",
            "b1d15a8979128c12b862809c6a56f932ea232dc2e4053f5970bb8c89b348460c",
        ),
        (
            "m2-fume-draw-miss-tide",
            "08a0d25f22df8d792b6c678172d848869d2b271fd419fcc1a5210549f791c7bc",
        ),
        (
            "m2-fume-late-manufacture",
            "a77f4378e8d7d31a6648772a8a99b34e1956956db3c6e615c84e347d8eddca17",
        ),
        (
            "m2-fume-late-repair",
            "a5da06e2699c9318354fa8985bd1025d8e5b7d6175fc74a3417a6f46e0259fd4",
        ),
        (
            "m2-fume-manufacture-bare",
            "e9e6e73c696d361f5cc46aebec300f66e55f99fff71b93b037b887cd261bb776",
        ),
        (
            "m2-fume-manufacture-local",
            "37b36690345bf6d1f6d06d5c8a47e7550f7d6f4998dc7e69ee241d058e036a11",
        ),
        (
            "m2-fume-manufacture-sale",
            "6cbbc696f13de987907ab9feaf7d6bb9e211780aca0dd7cc38500e5b3616b106",
        ),
        (
            "m2-fume-market-cask-delivered",
            "8d6a1138774d37df4a80e7ca6525705ed2fd57d226fac796ddf5a54f13956541",
        ),
        (
            "m2-fume-market-cask-undelivered",
            "2c213d5acc521fc89abcc275835b2841935cfa0a481dd23acc8f735c881075f3",
        ),
        (
            "m2-fume-market-water-composed",
            "59ad8bfdb03b2a01abfbd01d28738a85d88f2fb9ecd2d90f19f89ab3522e01f7",
        ),
        (
            "m2-fume-market-water",
            "fd82362e8e168f4065d60e9b638b4ea2623922b01b9c34dc74ae10b1f7dfddad",
        ),
        (
            "m2-fume-reclaim-charge",
            "ba82472e246524eb1e2ef3d073c7e6c495b699c4c0eb942bfaaa580464fc8c13",
        ),
        (
            "m2-fume-remote-spoil",
            "9e97e2a330e42388ab3cb791c7f46ee0479bf3a8bddc5c3e3b88e54b2c365b59",
        ),
        (
            "m2-fume-salvage-broken",
            "c36172f7bbebca3855d144bedc13640c99718a72db33d8e8aefe093f32f5f2c7",
        ),
        (
            "m2-fume-salvage-intact",
            "49891cdda73b826eba97a3ae5d2785b6f0cb669b9fc6e50228b88d1f518b8586",
        ),
        (
            "m2-fume-salvage-prior-filter-broken",
            "7e672087dc1c92111584f54c48395644bff7401bc4eb913fd87a427bb7c89a31",
        ),
        (
            "m2-fume-salvage-protected-production",
            "701b591b7918e2b4a1d962a12aa42fd1048241368217fbb53154a041b1a3caeb",
        ),
        (
            "m2-fume-salvage-reported",
            "0573151e9ad6663adc13cb8d7b4d6c1cb0c45a977526206c3cd3c1d1252d42fd",
        ),
        (
            "m2-fume-salvage-safe-local",
            "2a8a0f98fcdc5a921a0baea2f72d0deb2b432a5df2bc564ebf3ff88f909da204",
        ),
        (
            "m2-fume-salvage-safe-sale",
            "687f74516652a8e26021ed7014c7f01acdaf01166084c0f171fc7e31f9b3bb39",
        ),
        (
            "m2-fume-salvage-skilled",
            "a502d633a57ac36f2404cca85f1b474b785c8e0a1f99027fddacabb7b88044a0",
        ),
        (
            "m2-fume-salvage-unreported",
            "482c932b76eb277aea42091790c05328932f143d2a682907e0538a7f019c1afa",
        ),
        (
            "m2-fume-unscreened-freight",
            "87b3f822bb380150a69963fa1bfc6b3844382b6413c00b105a5e9f6a36cc4f11",
        ),
    ];
    assert_eq!(accepted.len(), 55);
    for (id, expected) in accepted {
        assert_eq!(
            binding(get(id).unwrap()).unwrap(),
            expected,
            "changed old binding {id}"
        );
    }
}
