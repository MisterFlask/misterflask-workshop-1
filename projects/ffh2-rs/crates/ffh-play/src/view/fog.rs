//! Fog rules (design §4.4): what the human may learn. These are the only visibility predicates
//! of the crate: the views and queries ask a [`Viewer`] (P3), the log drain a [`LogFog`] (P1).
//!
//! A [`Viewer`] answers for the human's team, or with the true state under debug reveal:
//! * plots: revealed (`Plot::revealed[team]`), visible (`plot_is_visible`); a revealed plot
//!   shows its current terrain and feature, the bonus only when the team's techs reveal it
//!   (`plot_get_bonus_type(p, team)`), and the improvement, route and owner the team last saw
//!   (`revealed_improvement_type`, `revealed_route_type`, `revealed_owner`);
//! * units: own units always; others only on visible plots and when not invisible to the team
//!   (`unit_is_invisible(u, team, true)`: C++'s default `bCheckCargo`, so a foreign transport's
//!   cargo is hidden), with their owner as `unit_get_visual_owner` shows it (hidden nationality);
//! * cities: own cities; foreign cities the team has revealed (`city_is_revealed`); their size
//!   and defence only while the city's plot is visible;
//! * players: the team's own players and the players of teams it has met.
//!
//! The log (below):
//!
//! ffh-core logs each C++ `addMessage` group once, tagged with one player or with none
//! (`LogEntry::player`). A tagged entry goes to that player only. An untagged entry stands for
//! messages C++ sends to *several* players under a condition the log does not record: players
//! who have met both teams (war, peace, vassals), players who have revealed the city (captured,
//! razed, liberated), voting members, everyone. Passing every untagged entry to the human leaked
//! wars, captures and razes among civilizations and cities it has never seen (P1 review).
//!
//! Until ffh-core tags recipients (a P7 item), [`LogFog`] recognizes the untagged messages by
//! their text (the formats of the ffh-core log sites) and evaluates their C++ condition for the
//! human when the log is drained; an untagged entry it does not recognize is held back (fail
//! closed), as are the event world news, whose sender the text does not name.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::log::LogEntry;
use ffh_data::*;

use crate::view::Viewer;

impl Viewer<'_> {
    /// The plot is on the human's map (revealed).
    pub fn plot_revealed(&self, p: PlotIdx) -> bool {
        self.reveal || self.g.map.plots[p].revealed[self.team]
    }

    /// The human's team sees the plot now.
    pub fn plot_visible(&self, p: PlotIdx) -> bool {
        self.reveal || self.g.plot_is_visible(p, self.team)
    }

    /// The plot's bonus as the human sees it: on a revealed plot, when its techs reveal it.
    pub fn plot_bonus(&self, p: PlotIdx) -> Option<BonusType> {
        if self.reveal {
            return self.g.map.plots[p].bonus_type;
        }
        if !self.plot_revealed(p) {
            return None;
        }
        self.g.plot_get_bonus_type(p, Some(self.team))
    }

    /// The improvement the human's team last saw on the plot.
    pub fn plot_improvement(&self, p: PlotIdx) -> Option<ImprovementType> {
        let pl = &self.g.map.plots[p];
        if self.reveal {
            pl.improvement_type
        } else {
            pl.revealed_improvement_type[self.team]
        }
    }

    /// The route the human's team last saw on the plot.
    pub fn plot_route(&self, p: PlotIdx) -> Option<RouteType> {
        let pl = &self.g.map.plots[p];
        if self.reveal {
            pl.route_type
        } else {
            pl.revealed_route_type[self.team]
        }
    }

    /// The owner the human's team last saw for the plot.
    pub fn plot_owner(&self, p: PlotIdx) -> Option<PlayerId> {
        let pl = &self.g.map.plots[p];
        if self.reveal {
            pl.owner
        } else {
            pl.revealed_owner[self.team]
        }
    }

    /// The human sees unit `u` (C++: on a visible plot and `!isInvisible(team)`).
    pub fn unit_visible(&self, u: UnitId) -> bool {
        let g = self.g;
        let Some(unit) = g.units.get(u) else { return false };
        if self.reveal || unit.owner == self.h {
            return true;
        }
        let Some(p) = g.map.plot_xy(unit.x, unit.y) else { return false };
        self.plot_visible(p) && !g.unit_is_invisible(u, self.team, true)
    }

    /// The owner of `u` as the human sees it: hidden nationality shows foreign units as
    /// barbarians (`getVisualOwner`).
    pub fn unit_owner(&self, u: UnitId) -> PlayerId {
        let g = self.g;
        if self.reveal || g.units[u].owner == self.h {
            return g.units[u].owner;
        }
        g.unit_get_visual_owner(u, Some(self.team))
    }

    /// The city is on the human's map.
    pub fn city_revealed(&self, c: CityId) -> bool {
        let g = self.g;
        g.cities.get(c).is_some_and(|city| self.reveal || city.owner == self.h || g.city_is_revealed(c, self.team, false))
    }

    /// The human sees the city now (its plot is visible).
    pub fn city_visible(&self, c: CityId) -> bool {
        let g = self.g;
        g.cities.get(c).is_some_and(|city| city.owner == self.h || self.plot_visible(g.city_plot(c)))
    }

    /// The human's team is `t` or has met it.
    pub fn team_met(&self, t: TeamId) -> bool {
        self.reveal || t == self.team || self.g.teams[self.team].has_met[t]
    }

    /// The human knows player `p` (its team met `p`'s).
    pub fn player_met(&self, p: PlayerId) -> bool {
        self.team_met(self.g.players[p].team)
    }
}

/// The log filter of one human player.
#[derive(Clone, Debug, Default)]
pub struct LogFog {
    /// Names of the cities the human's team had revealed at the previous drain: a city captured
    /// or razed since may be gone (or renamed by its new owner) when the entry is drained.
    revealed_cities: Vec<String>,
}

/// A team id as the log prints it (`TeamId`'s `Display`).
fn parse_team(g: &Game, s: &str) -> Option<TeamId> {
    s.parse::<u8>().ok().filter(|&t| (t as usize) < g.num_teams()).map(TeamId)
}

/// The two teams of a "Team A <relation> team B" message (team/diplomacy.rs).
fn two_teams(g: &Game, text: &str) -> Option<(TeamId, TeamId)> {
    const RELATIONS: [&str; 6] = [
        "declared war on team ",
        "made peace with team ",
        "capitulated to team ",
        "became a vassal of team ",
        "revolted against its surrender to team ",
        "revolted against its master team ",
    ];
    let rest = text.strip_prefix("Team ")?;
    let (a, rest) = rest.split_once(' ')?;
    let b = RELATIONS.iter().find_map(|r| rest.strip_prefix(r))?;
    Some((parse_team(g, a)?, parse_team(g, b)?))
}

fn city_name(g: &Game, c: CityId) -> &str {
    g.d.text.name(&g.cities[c].name)
}

impl LogFog {
    /// The filter for `h` in game `g` (a new or loaded session).
    pub fn new(g: &Game, h: PlayerId) -> LogFog {
        let mut fog = LogFog::default();
        fog.update(g, h);
        fog
    }

    /// After a drain: remembers the cities the human's team has revealed now.
    pub fn update(&mut self, g: &Game, h: PlayerId) {
        let team = g.players[h].team;
        self.revealed_cities.clear();
        self.revealed_cities.extend(g.cities.iter().filter(|(_, c)| c.revealed[team]).map(|(c, _)| city_name(g, c).to_string()));
    }

    /// Did the human's team reveal a city whose message `text` starts with its name followed by
    /// `after` (at the previous drain, or now)?
    fn about_revealed_city(&self, g: &Game, h: PlayerId, text: &str, after: &str) -> bool {
        let team = g.players[h].team;
        let named = |name: &str| text.strip_prefix(name).is_some_and(|rest| rest.starts_with(after));
        self.revealed_cities.iter().any(|n| named(n)) || g.cities.iter().any(|(c, city)| city.revealed[team] && named(city_name(g, c)))
    }

    /// The entry as the human `h` receives it, or `None` when C++ would not send it to `h`.
    pub fn filter(&self, g: &Game, h: PlayerId, e: LogEntry) -> Option<LogEntry> {
        if let Some(p) = e.player {
            return (p == h.0).then_some(e);
        }
        let d = g.d;
        let team = g.players[h].team;
        let met = |t: TeamId| t == team || g.teams[team].has_met[t];
        let text = e.text.as_str();
        // CvTeam::declareWar / makePeace / setVassal: the two teams' players and those who have
        // met both.
        if let Some((a, b)) = two_teams(g, text) {
            return (met(a) && met(b)).then_some(e);
        }
        // CvTeam::setCircumnavigated: the team, those who met it, and "an unknown civilization"
        // for the others.
        if let Some(rest) = text.strip_prefix("Team ") {
            if let Some((a, tail)) = rest.split_once(' ') {
                if tail.starts_with("has circumnavigated the globe") {
                    let a = parse_team(g, a)?;
                    if met(a) {
                        return Some(e);
                    }
                    return Some(LogEntry { text: "An unknown civilization has circumnavigated the globe!".into(), ..e });
                }
            }
        }
        // CvPlayer::acquireCity / raze, CvCity::liberate: the players who revealed the city.
        if text.contains(") was captured by the ") {
            return self.about_revealed_city(g, h, text, " (").then_some(e);
        }
        if text.contains(" was razed by the ") {
            return self.about_revealed_city(g, h, text, " was razed by the ").then_some(e);
        }
        if text.contains(" liberated: player ") {
            return self.about_revealed_city(g, h, text, " liberated: player ").then_some(e);
        }
        // CvGame::doVoteResults: the result goes to the voting members.
        for vs in d.vote_sources.keys() {
            let source = d.text.name(&d.vote_sources[vs].base.description);
            if text.strip_prefix(source).is_some_and(|r| r.starts_with(": ")) && (text.ends_with(" passed") || text.ends_with(" failed")) {
                return g.player_is_voting_member(h, vs).then_some(e);
            }
        }
        // Messages to every alive player: setVoteSourceReligion, makeSpecialBuildingValid, nukes.
        let everyone = d.religions.keys().any(|r| {
            let religion = d.text.name(&d.religions[r].base.description);
            d.vote_sources.keys().any(|vs| text == format!("{religion} is now the religion of the {}", d.text.name(&d.vote_sources[vs].base.description)))
        }) || d.special_buildings.keys().any(|sb| text == format!("{} can now be built", d.text.name(&d.special_buildings[sb].base.description)))
            || (text.starts_with("A nuke of player ") && text.contains(" was intercepted by team "))
            || (text.starts_with("Player ") && text.contains(" launched a nuke at ("));
        // Anything else untagged (event world news, new log sites) is held back.
        everyone.then_some(e)
    }
}
