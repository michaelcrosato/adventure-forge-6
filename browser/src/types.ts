export type DecimalString = string;

export interface PresetOption {
  id: string;
  display_name: string;
  summary: string;
}

export interface ChoiceOption {
  id: string;
  display_name: string;
  summary: string;
}

export interface CreationSlot {
  id: string;
  order: DecimalString;
  display_name: string;
  choices: ChoiceOption[];
}

export interface StartOptions {
  build_id: string;
  presets: PresetOption[];
  creation_slots: CreationSlot[];
}

export interface CharacterChoiceSelection {
  slot_id: string;
  choice_id: string;
}

export interface CharacterSelection {
  name: string;
  choices: CharacterChoiceSelection[];
}

export type StartRecipe =
  | {
      kind: "preset";
      character_preset_id: string;
      seed: DecimalString;
    }
  | {
      kind: "custom";
      selection: CharacterSelection;
      seed: DecimalString;
    };

export interface TimedEventView {
  label: string;
  remaining_ticks: DecimalString;
}

export interface ResourceView {
  id: string;
  name: string;
  amount: DecimalString;
}

export interface ItemView {
  id: string;
  name: string;
  count: DecimalString;
}

export interface SupplyView {
  resources: ResourceView[];
  items: ItemView[];
}

export interface Observation {
  build_id: string;
  state_id: string;
  location_id: string;
  title: string;
  text: string;
  supplies: SupplyView;
  result: string | null;
  world_time: DecimalString;
  upcoming_events: TimedEventView[];
  action_set_digest: string;
  action_count: DecimalString;
}

export interface ActionTimeCost {
  minimum_ticks: DecimalString;
  maximum_ticks: DecimalString;
}

export interface ActionView {
  action_id: string;
  definition_id: string;
  label: string;
  category: string;
  time_cost: ActionTimeCost;
  consequence_preview: string | null;
  parameter_display_values: Record<string, string>;
  parameters: Record<string, string>;
}

export interface ActionPage {
  build_id: string;
  state_id: string;
  actions: ActionView[];
  total: DecimalString;
  digest: string;
  offset: DecimalString;
  next_offset: DecimalString | null;
}

export interface SessionView {
  revision: DecimalString;
  observation: Observation;
  catalog: ActionPage;
}

export interface SessionHandle {
  session_id: string;
  view: SessionView;
}

export interface BootstrapResponse {
  token: string;
  instance_id: string;
}

export interface CurrentResponse {
  session: SessionHandle | null;
}

export interface ActionRequest {
  command_id: string;
  expected_revision: DecimalString;
  expected_state_id: string;
  action_id: string;
}

export interface ClientState {
  readonly phase:
    | "booting"
    | "start"
    | "ready"
    | "working"
    | "uncertain"
    | "closed"
    | "error"
    | "restarted";
  readonly options: StartOptions | null;
  readonly session: SessionHandle | null;
  readonly actions: readonly ActionView[];
  readonly catalogComplete: boolean;
  readonly message: string | null;
  readonly storageWarning: string | null;
  readonly pendingLabel: string | null;
}
