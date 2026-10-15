//! One game's settings, as type names (so they print and pass on a command line as is).

use ffh_core::setup::{SetupOptions, TeamSetup};
use ffh_data::*;

#[derive(Clone, Debug)]
pub struct Settings {
    /// Matrix group this game belongs to (summary rows); not a game setting.
    pub group: String,
    pub seed: u32,
    pub civs: usize,
    pub world: String,
    pub speed: String,
    pub handicap: String,
    pub player_handicap: String,
    pub era: String,
    pub climate: String,
    pub sea: String,
    pub options: Vec<String>,
    /// Consecutive civs in teams of this size (0 or 1 = every civ alone).
    pub teams: usize,
    pub turns: i32,
    pub max_turns: i32,
    /// Score victory target in basis points of the world (score v2; 5000 = half), 0 = off.
    pub target_score: i32,
    pub max_city_elimination: i32,
    pub no_victory: Vec<String>,
    /// Open slots after the civs (None = the standard count).
    pub spare: Option<usize>,
    /// `VICTORY_X@T`: at turn T, the capital of the first alive civ gets every building whose
    /// class has a victory threshold for VICTORY_X (forces the FfH tower/altar victories).
    pub grant_victory: Option<String>,
}

impl Settings {
    /// The standard game (`GameSetup::standard`) on a small map.
    pub fn base(group: &str, seed: u32, turns: i32) -> Settings {
        Settings {
            group: group.into(),
            seed,
            civs: 5,
            world: "WORLDSIZE_SMALL".into(),
            speed: "GAMESPEED_NORMAL".into(),
            handicap: "HANDICAP_NOBLE".into(),
            player_handicap: "HANDICAP_NOBLE".into(),
            era: "ERA_ANCIENT".into(),
            climate: "CLIMATE_TEMPERATE".into(),
            sea: "SEALEVEL_MEDIUM".into(),
            options: Vec::new(),
            teams: 0,
            turns,
            max_turns: 0,
            target_score: 0,
            max_city_elimination: 0,
            no_victory: Vec::new(),
            spare: None,
            grant_victory: None,
        }
    }

    pub fn parse(args: &[String]) -> Result<Settings, String> {
        let mut s = Settings::base("run", 1, 400);
        let mut i = 0;
        while i < args.len() {
            let v = args.get(i + 1).cloned().ok_or_else(|| format!("{} needs a value", args[i]))?;
            let num = |v: &str| v.parse::<i64>().map_err(|_| format!("{}: not a number: {v}", args[i]));
            let list = |v: &str| v.split(',').filter(|x| !x.is_empty()).map(String::from).collect::<Vec<_>>();
            match args[i].as_str() {
                "--group" => s.group = v,
                "--seed" => s.seed = num(&v)? as u32,
                "--civs" => s.civs = num(&v)? as usize,
                "--size" => s.world = v,
                "--speed" => s.speed = v,
                "--handicap" => s.handicap = v,
                "--player-handicap" => s.player_handicap = v,
                "--era" => s.era = v,
                "--climate" => s.climate = v,
                "--sea" => s.sea = v,
                "--options" => s.options = list(&v),
                "--teams" => s.teams = num(&v)? as usize,
                "--turns" => s.turns = num(&v)? as i32,
                "--max-turns" => s.max_turns = num(&v)? as i32,
                "--target-score" => s.target_score = num(&v)? as i32,
                "--max-city-elimination" => s.max_city_elimination = num(&v)? as i32,
                "--no-victory" => s.no_victory = list(&v),
                "--spare" => s.spare = Some(num(&v)? as usize),
                "--grant-victory" => s.grant_victory = Some(v),
                a => return Err(format!("unknown option {a}")),
            }
            i += 2;
        }
        Ok(s)
    }

    /// Command-line form (inverse of `parse`); default-valued settings are left out.
    pub fn to_args(&self) -> Vec<String> {
        let b = Settings::base("", 0, 0);
        let mut a: Vec<String> = Vec::new();
        let mut push = |k: &str, v: String| {
            a.push(k.into());
            a.push(v);
        };
        push("--group", self.group.clone());
        push("--seed", self.seed.to_string());
        push("--civs", self.civs.to_string());
        push("--size", self.world.clone());
        push("--turns", self.turns.to_string());
        for (k, v, dflt) in [
            ("--speed", &self.speed, &b.speed),
            ("--handicap", &self.handicap, &b.handicap),
            ("--player-handicap", &self.player_handicap, &b.player_handicap),
            ("--era", &self.era, &b.era),
            ("--climate", &self.climate, &b.climate),
            ("--sea", &self.sea, &b.sea),
        ] {
            if v != dflt {
                push(k, v.clone());
            }
        }
        if !self.options.is_empty() {
            push("--options", self.options.join(","));
        }
        if self.teams > 1 {
            push("--teams", self.teams.to_string());
        }
        for (k, v) in [("--max-turns", self.max_turns), ("--target-score", self.target_score), ("--max-city-elimination", self.max_city_elimination)] {
            if v != 0 {
                push(k, v.to_string());
            }
        }
        if !self.no_victory.is_empty() {
            push("--no-victory", self.no_victory.join(","));
        }
        if let Some(n) = self.spare {
            push("--spare", n.to_string());
        }
        if let Some(g) = &self.grant_victory {
            push("--grant-victory", g.clone());
        }
        a
    }

    /// Short one-line description (prefixes like `GAMESPEED_` dropped).
    pub fn describe(&self) -> String {
        let short = |s: &str| s.split_once('_').map_or(s.to_string(), |(_, r)| r.to_string());
        let mut out = format!(
            "s{} {}x{} {} {} {} {} {} {}",
            self.seed,
            short(&self.world),
            self.civs,
            short(&self.speed),
            short(&self.handicap),
            short(&self.era),
            short(&self.climate),
            short(&self.sea),
            self.turns
        );
        if self.player_handicap != "HANDICAP_NOBLE" {
            out += &format!(" ai={}", short(&self.player_handicap));
        }
        if self.teams > 1 {
            out += &format!(" teams={}", self.teams);
        }
        if !self.options.is_empty() {
            out += &format!(" [{}]", self.options.iter().map(|o| short(o)).collect::<Vec<_>>().join(","));
        }
        if self.max_turns != 0 {
            out += &format!(" maxturns={}", self.max_turns);
        }
        if self.target_score != 0 {
            out += &format!(" target={}", self.target_score);
        }
        if self.max_city_elimination != 0 {
            out += &format!(" maxcityelim={}", self.max_city_elimination);
        }
        if !self.no_victory.is_empty() {
            out += &format!(" novic={}", self.no_victory.iter().map(|o| short(o)).collect::<Vec<_>>().join(","));
        }
        if let Some(n) = self.spare {
            out += &format!(" spare={n}");
        }
        if let Some(g) = &self.grant_victory {
            out += &format!(" grant={}", short(g));
        }
        out
    }

    /// The `SetupOptions` for these settings (unknown type names are errors).
    pub fn setup_options(&self, d: &GameData) -> Result<SetupOptions, String> {
        fn ty<T: InfoId>(d: &GameData, name: &str) -> Result<T, String> {
            d.ty::<T>(name).ok_or_else(|| format!("unknown {} type {name}", T::CLASS))
        }
        let _: WorldSizeType = ty(d, &self.world)?;
        let mut o = SetupOptions::new(d, self.civs, &self.world, self.seed);
        o.game_speed = ty(d, &self.speed)?;
        o.handicap = ty(d, &self.handicap)?;
        o.player_handicap = ty(d, &self.player_handicap)?;
        o.era = ty(d, &self.era)?;
        o.climate = ty(d, &self.climate)?;
        o.sea_level = ty(d, &self.sea)?;
        o.options = self.options.iter().map(|n| ty::<GameOptionType>(d, n)).collect::<Result<_, _>>()?;
        o.disabled_victories = self.no_victory.iter().map(|n| ty::<VictoryType>(d, n)).collect::<Result<_, _>>()?;
        o.max_turns = self.max_turns;
        o.target_score = self.target_score;
        o.max_city_elimination = self.max_city_elimination;
        o.teams = if self.teams > 1 { TeamSetup::TeamsOf(self.teams) } else { TeamSetup::Solo };
        if let Some(n) = self.spare {
            o.spare_slots = n;
        }
        if self.civs == 0 || self.civs > ffh_core::GameSetup::MAX_CIVS {
            return Err(format!("civs must be in 1..={}", ffh_core::GameSetup::MAX_CIVS));
        }
        Ok(o)
    }
}
