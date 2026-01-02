# Claude Code Instructions for FFH Simplified

## Before Returning Control to User

**IMPORTANT**: Before completing any task that modifies game code, run the following command to verify the game still functions:

```bash
source ~/.nvm/nvm.sh && cd /home/ironlordbyron/general-knowledge-base/projects/ffh-simplified && npm run test
```

All 12 unit tests should pass. If any tests fail, fix the issues before returning control to the user.

## Playing the Game (CLI Mode)

Claude can play the game directly via the CLI interface:

```bash
source ~/.nvm/nvm.sh && cd /home/ironlordbyron/general-knowledge-base/projects/ffh-simplified && npm run cli
```

### CLI Commands
- `select L1` - Select your legion L1
- `move X Y` - Move selected legion to coordinates (X,Y)
- `attack E1` - Attack enemy legion E1
- `end` - End your turn
- `status` - Show full game status
- `help` - Show all commands
- `quit` - Exit game

### Map Legend
- `.` = Grassland, `♣` = Forest, `^` = Hills, `▲` = Mountain, `~` = Water
- `⌂` = City (cyan = yours, red = enemy)
- `1-9` = Legion (cyan = yours, red = enemy)
- `L1, L2...` = Your legions, `E1, E2...` = Enemy legions

## Available Commands

- `npm run dev` - Start development server (localhost:3000)
- `npm run cli` - Play the game in CLI mode (for Claude to play)
- `npm run test` - Run unit tests with Vitest
- `npm run test:e2e` - Run E2E tests with Playwright (requires system deps)
- `npm run build` - Build for production

## Project Structure

- `src/game/` - Core game logic (Game.ts, Combat.ts, MapGenerator.ts)
- `src/rendering/` - Canvas rendering (Renderer.ts)
- `src/data/` - Game data (soldiers, buildings, factions)
- `src/types.ts` - TypeScript type definitions
- `e2e/` - Playwright E2E tests
- `assets/` - Sprites and visual assets

## WSL Notes

This project runs on WSL. Use `source ~/.nvm/nvm.sh` before npm commands to ensure the Linux Node.js is used instead of Windows Node.js.
