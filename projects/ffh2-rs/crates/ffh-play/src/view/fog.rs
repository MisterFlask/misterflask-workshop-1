//! Fog rules (design §4.4): what the human may learn. P1: the game log.
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
