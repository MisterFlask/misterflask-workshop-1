import * as readline from 'readline';
import {
  createInitialGameState,
  processAction,
  getValidMoves,
  getTile,
  getLegionAt,
  getCityAt,
  getPlayerLegions,
  getPlayerCities,
  getFactionLegions,
} from './game/Game';
import { SOLDIER_TYPES } from './data/soldiers';
import { BUILDING_TYPES } from './data/buildings';
import type { GameState, Coord, Legion, City } from './types';

// ANSI color codes
const colors = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',

  black: '\x1b[30m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
  white: '\x1b[37m',

  bgBlack: '\x1b[40m',
  bgRed: '\x1b[41m',
  bgGreen: '\x1b[42m',
  bgYellow: '\x1b[43m',
  bgBlue: '\x1b[44m',
  bgMagenta: '\x1b[45m',
  bgCyan: '\x1b[46m',
  bgWhite: '\x1b[47m',
};

function c(color: keyof typeof colors, text: string): string {
  return `${colors[color]}${text}${colors.reset}`;
}

class GameCLI {
  private state: GameState;
  private rl: readline.Interface;
  private selectedLegionId: string | null = null;
  private legionLabels: Map<string, string> = new Map();
  private labelToLegion: Map<string, string> = new Map();

  constructor() {
    this.state = createInitialGameState(Date.now());
    this.rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    this.assignLegionLabels();
  }

  private assignLegionLabels(): void {
    this.legionLabels.clear();
    this.labelToLegion.clear();

    let playerNum = 1;
    let enemyNum = 1;

    for (const legion of this.state.legions.values()) {
      let label: string;
      if (legion.owner === 'player') {
        label = `L${playerNum++}`;
      } else {
        label = `E${enemyNum++}`;
      }
      this.legionLabels.set(legion.id, label);
      this.labelToLegion.set(label.toLowerCase(), legion.id);
    }
  }

  private renderMap(): string {
    const lines: string[] = [];
    const { mapWidth, mapHeight } = this.state;

    // Render viewport (show relevant portion)
    const viewWidth = Math.min(mapWidth, 40);
    const viewHeight = Math.min(mapHeight, 20);

    // Header with column numbers
    let header = '   ';
    for (let x = 0; x < viewWidth; x++) {
      header += (x % 10).toString();
    }
    lines.push(c('dim', header));
    lines.push(c('dim', '   ' + '─'.repeat(viewWidth)));

    for (let y = 0; y < viewHeight; y++) {
      let row = c('dim', `${y.toString().padStart(2)} │`);

      for (let x = 0; x < viewWidth; x++) {
        const coord = { x, y };
        const tile = getTile(this.state, coord);
        const legion = getLegionAt(this.state, coord);
        const city = getCityAt(this.state, coord);

        let char = '.';
        let color: keyof typeof colors = 'dim';

        if (tile) {
          // Terrain
          switch (tile.terrain) {
            case 'grass': char = '.'; color = 'green'; break;
            case 'forest': char = '♣'; color = 'green'; break;
            case 'hills': char = '^'; color = 'yellow'; break;
            case 'mountain': char = '▲'; color = 'white'; break;
            case 'water': char = '~'; color = 'blue'; break;
          }
        }

        // City overrides terrain
        if (city) {
          if (city.owner === 'player') {
            char = '⌂';
            color = 'cyan';
          } else {
            char = '⌂';
            color = 'red';
          }
        }

        // Legion overrides all
        if (legion) {
          const label = this.legionLabels.get(legion.id) || '?';
          if (legion.owner === 'player') {
            color = 'cyan';
            if (legion.id === this.selectedLegionId) {
              row += c('bold', c('cyan', label.charAt(label.length - 1)));
            } else {
              row += c(color, label.charAt(label.length - 1));
            }
            continue;
          } else {
            char = label.charAt(label.length - 1);
            color = 'red';
          }
        }

        row += c(color, char);
      }

      lines.push(row);
    }

    return lines.join('\n');
  }

  private renderStatus(): string {
    const player = this.state.factions.get('player')!;
    const lines: string[] = [];

    lines.push('');
    lines.push(c('bold', `═══ Turn ${this.state.turn} ═══`));
    lines.push(
      `Gold: ${c('yellow', player.gold.toString())} | ` +
      `Mana: ${c('magenta', player.mana.toString())} | ` +
      `Armageddon: ${c('red', `${this.state.armageddonCounter}/100`)}`
    );

    return lines.join('\n');
  }

  private renderLegions(): string {
    const lines: string[] = [];
    const playerLegions = getPlayerLegions(this.state);

    lines.push('');
    lines.push(c('bold', '─── Your Legions ───'));

    for (const legion of playerLegions) {
      const label = this.legionLabels.get(legion.id) || '?';
      const selected = legion.id === this.selectedLegionId ? c('bold', ' [SELECTED]') : '';
      const soldierSummary = this.getSoldierSummary(legion);

      lines.push(
        `${c('cyan', label)}: (${legion.location.x},${legion.location.y}) ` +
        `Move: ${legion.movementRemaining}/3 | ${soldierSummary}${selected}`
      );
    }

    // Show enemy legions
    const enemyLegions = Array.from(this.state.legions.values()).filter(l => l.owner !== 'player');
    if (enemyLegions.length > 0) {
      lines.push('');
      lines.push(c('bold', '─── Enemy Legions ───'));
      for (const legion of enemyLegions) {
        const label = this.legionLabels.get(legion.id) || '?';
        const soldierSummary = this.getSoldierSummary(legion);
        lines.push(
          `${c('red', label)}: (${legion.location.x},${legion.location.y}) | ` +
          `${c('dim', legion.owner)} | ${soldierSummary}`
        );
      }
    }

    return lines.join('\n');
  }

  private renderCities(): string {
    const lines: string[] = [];
    const playerCities = getPlayerCities(this.state);

    lines.push('');
    lines.push(c('bold', '─── Your Cities ───'));

    for (const city of playerCities) {
      const buildings = city.buildings.length > 0 ? city.buildings.join(', ') : 'none';
      const occupied = city.occupationTurns > 0 ? c('red', ` [Occupied ${city.occupationTurns}t]`) : '';
      lines.push(
        `${c('cyan', city.name)}: (${city.coord.x},${city.coord.y}) ` +
        `Pop: ${city.population} | Buildings: ${buildings}${occupied}`
      );
    }

    return lines.join('\n');
  }

  private getSoldierSummary(legion: Legion): string {
    const counts: Record<string, number> = {};
    let totalHp = 0;
    let maxHp = 0;

    for (const s of legion.soldiers) {
      counts[s.type] = (counts[s.type] || 0) + 1;
      totalHp += s.hp;
      maxHp += s.maxHp;
    }

    const parts = Object.entries(counts).map(([t, n]) => `${n}${t.charAt(0).toUpperCase()}`);
    return `${legion.soldiers.length}/8 [${parts.join(' ')}] HP:${totalHp}/${maxHp}`;
  }

  private renderSelectedLegion(): string {
    if (!this.selectedLegionId) return '';

    const legion = this.state.legions.get(this.selectedLegionId);
    if (!legion) return '';

    const lines: string[] = [];
    const validMoves = getValidMoves(this.state, legion);

    lines.push('');
    lines.push(c('bold', '─── Selected Legion ───'));
    lines.push(`Position: (${legion.location.x},${legion.location.y}) | Movement: ${legion.movementRemaining}/3`);
    lines.push('Soldiers:');

    for (const s of legion.soldiers) {
      const type = SOLDIER_TYPES[s.type];
      lines.push(
        `  ${s.type}: HP ${s.hp}/${s.maxHp} | ATK ${type.attack} | DEF ${type.defense} | ` +
        `Row: ${s.position.row}`
      );
    }

    if (validMoves.length > 0 && legion.movementRemaining > 0) {
      const moveList = validMoves.slice(0, 10).map(m => `(${m.x},${m.y})`).join(' ');
      const more = validMoves.length > 10 ? ` +${validMoves.length - 10} more` : '';
      lines.push(`Valid moves: ${moveList}${more}`);
    }

    return lines.join('\n');
  }

  private renderHelp(): string {
    return `
${c('bold', '─── Commands ───')}
  ${c('cyan', 'select L1')}      - Select legion L1
  ${c('cyan', 'move X Y')}       - Move selected legion to (X,Y)
  ${c('cyan', 'attack E1')}      - Attack enemy legion E1
  ${c('cyan', 'end')}            - End turn
  ${c('cyan', 'status')}         - Show full status
  ${c('cyan', 'help')}           - Show this help
  ${c('cyan', 'quit')}           - Exit game
`;
  }

  private render(): void {
    console.clear();
    console.log(this.renderStatus());
    console.log(this.renderMap());
    console.log(this.renderLegions());
    console.log(this.renderCities());
    console.log(this.renderSelectedLegion());
  }

  private parseCommand(input: string): { command: string; args: string[] } {
    const parts = input.trim().toLowerCase().split(/\s+/);
    return {
      command: parts[0] || '',
      args: parts.slice(1),
    };
  }

  private handleCommand(input: string): string {
    const { command, args } = this.parseCommand(input);

    switch (command) {
      case 'help':
      case 'h':
      case '?':
        return this.renderHelp();

      case 'select':
      case 'sel':
      case 's':
        return this.cmdSelect(args);

      case 'move':
      case 'm':
        return this.cmdMove(args);

      case 'attack':
      case 'a':
        return this.cmdAttack(args);

      case 'end':
      case 'e':
      case 'done':
        return this.cmdEndTurn();

      case 'status':
      case 'st':
        return this.cmdStatus();

      case 'quit':
      case 'exit':
      case 'q':
        console.log('Goodbye!');
        process.exit(0);

      case '':
        return '';

      default:
        return c('red', `Unknown command: ${command}. Type 'help' for commands.`);
    }
  }

  private cmdSelect(args: string[]): string {
    if (args.length < 1) {
      return c('red', 'Usage: select L1');
    }

    const label = args[0].toLowerCase();
    const legionId = this.labelToLegion.get(label);

    if (!legionId) {
      return c('red', `Unknown legion: ${args[0]}`);
    }

    const legion = this.state.legions.get(legionId);
    if (!legion) {
      return c('red', `Legion not found: ${args[0]}`);
    }

    if (legion.owner !== 'player') {
      return c('red', `Cannot select enemy legion. Use 'attack ${args[0]}' instead.`);
    }

    this.selectedLegionId = legionId;
    return c('green', `Selected ${args[0].toUpperCase()}`);
  }

  private cmdMove(args: string[]): string {
    if (!this.selectedLegionId) {
      return c('red', 'No legion selected. Use: select L1');
    }

    if (args.length < 2) {
      return c('red', 'Usage: move X Y');
    }

    const x = parseInt(args[0], 10);
    const y = parseInt(args[1], 10);

    if (isNaN(x) || isNaN(y)) {
      return c('red', 'Invalid coordinates. Usage: move X Y');
    }

    const legion = this.state.legions.get(this.selectedLegionId);
    if (!legion) {
      return c('red', 'Selected legion no longer exists.');
    }

    if (legion.movementRemaining <= 0) {
      return c('red', 'Legion has no movement remaining this turn.');
    }

    const validMoves = getValidMoves(this.state, legion);
    const isValid = validMoves.some(m => m.x === x && m.y === y);

    if (!isValid) {
      return c('red', `Cannot move to (${x},${y}). Not a valid move.`);
    }

    // Check for combat
    const enemyLegion = getLegionAt(this.state, { x, y });
    if (enemyLegion && enemyLegion.owner !== 'player') {
      const enemyLabel = this.legionLabels.get(enemyLegion.id) || '?';
      const oldState = this.state;

      this.state = processAction(this.state, {
        type: 'move_legion',
        legionId: this.selectedLegionId,
        to: { x, y },
      });

      this.assignLegionLabels();

      // Check combat result
      const attackerSurvived = this.state.legions.has(this.selectedLegionId);
      const defenderSurvived = this.state.legions.has(enemyLegion.id);

      if (!defenderSurvived && attackerSurvived) {
        this.selectedLegionId = null;
        return c('green', `Victory! Defeated ${enemyLabel} at (${x},${y})`);
      } else if (!attackerSurvived) {
        this.selectedLegionId = null;
        return c('red', `Defeat! Your legion was destroyed attacking ${enemyLabel}`);
      } else {
        this.selectedLegionId = null;
        return c('yellow', `Battle with ${enemyLabel}. Both sides took casualties.`);
      }
    }

    // Normal move
    this.state = processAction(this.state, {
      type: 'move_legion',
      legionId: this.selectedLegionId,
      to: { x, y },
    });

    // Check for city capture
    const city = getCityAt(this.state, { x, y });
    if (city && city.owner === 'player') {
      // Check if we just captured it
      const oldCity = getCityAt({ ...this.state, cities: new Map() }, { x, y });
      if (city.occupationTurns > 0) {
        this.selectedLegionId = null;
        return c('green', `Captured ${city.name}! (Occupied for ${city.occupationTurns} turns)`);
      }
    }

    const newLegion = this.state.legions.get(this.selectedLegionId);
    const remaining = newLegion?.movementRemaining ?? 0;
    this.selectedLegionId = null;

    return c('green', `Moved to (${x},${y}). Movement remaining: ${remaining}/3`);
  }

  private cmdAttack(args: string[]): string {
    if (!this.selectedLegionId) {
      return c('red', 'No legion selected. Use: select L1');
    }

    if (args.length < 1) {
      return c('red', 'Usage: attack E1');
    }

    const label = args[0].toLowerCase();
    const enemyLegionId = this.labelToLegion.get(label);

    if (!enemyLegionId) {
      return c('red', `Unknown legion: ${args[0]}`);
    }

    const enemyLegion = this.state.legions.get(enemyLegionId);
    if (!enemyLegion) {
      return c('red', `Legion not found: ${args[0]}`);
    }

    if (enemyLegion.owner === 'player') {
      return c('red', `Cannot attack your own legion!`);
    }

    // Check if enemy is in range
    const legion = this.state.legions.get(this.selectedLegionId)!;
    const validMoves = getValidMoves(this.state, legion);
    const canReach = validMoves.some(
      m => m.x === enemyLegion.location.x && m.y === enemyLegion.location.y
    );

    if (!canReach) {
      return c('red', `${args[0].toUpperCase()} is not in range. Move closer first.`);
    }

    // Attack = move to enemy location
    return this.cmdMove([
      enemyLegion.location.x.toString(),
      enemyLegion.location.y.toString(),
    ]);
  }

  private cmdEndTurn(): string {
    this.state = processAction(this.state, { type: 'end_turn' });
    this.selectedLegionId = null;
    this.assignLegionLabels();

    if (this.state.gameOver) {
      if (this.state.winner === 'player') {
        return c('green', '*** VICTORY! You have won the game! ***');
      } else {
        return c('red', '*** DEFEAT! Your empire has fallen. ***');
      }
    }

    return c('green', `Turn ${this.state.turn} begins.`);
  }

  private cmdStatus(): string {
    const lines: string[] = [];
    const player = this.state.factions.get('player')!;

    lines.push(c('bold', '\n═══ Full Status ═══'));
    lines.push(`Turn: ${this.state.turn}`);
    lines.push(`Gold: ${player.gold} | Mana: ${player.mana}`);
    lines.push(`Armageddon Counter: ${this.state.armageddonCounter}/100`);
    lines.push(`Phase: ${this.state.phase}`);
    lines.push(`Your Legions: ${getPlayerLegions(this.state).length}`);
    lines.push(`Your Cities: ${getPlayerCities(this.state).length}`);

    return lines.join('\n');
  }

  async run(): Promise<void> {
    console.log(c('bold', '\n╔═══════════════════════════════════╗'));
    console.log(c('bold', '║     FFH SIMPLIFIED - CLI MODE     ║'));
    console.log(c('bold', '╚═══════════════════════════════════╝'));
    console.log("\nType 'help' for commands.\n");

    this.render();

    const prompt = (): void => {
      const promptText = this.selectedLegionId
        ? `[${this.legionLabels.get(this.selectedLegionId)}] > `
        : '> ';

      this.rl.question(promptText, (input) => {
        const result = this.handleCommand(input);
        if (result) {
          console.log(result);
        }

        if (!['help', 'h', '?', 'status', 'st'].includes(input.trim().toLowerCase().split(/\s+/)[0])) {
          this.render();
        }

        if (this.state.gameOver) {
          console.log('\nGame Over. Press Ctrl+C to exit.');
        }

        prompt();
      });
    };

    prompt();
  }
}

// Start the game
const game = new GameCLI();
game.run().catch(console.error);
