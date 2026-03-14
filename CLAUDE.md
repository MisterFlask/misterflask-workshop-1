# Claude Code Project Instructions

## FFH Simplified Game Project

### Design Document Synchronization

**IMPORTANT**: When making any changes to gameplay mechanics in the code, you MUST also update the corresponding design documentation to reflect those changes.

Design documents are located in:
- `ffh-simplified-design/FFH_SIMPLIFIED_MASTER_DESIGN.md` - Main game design document
- `projects/ffh-simplified/docs/UNIT_DESIGN.md` - Unit design details

When modifying gameplay mechanics such as:
- Unit stats, costs, or abilities
- Building effects or requirements
- Terrain features and their effects
- Combat mechanics
- Economy/resource values
- Armageddon counter thresholds
- Movement costs
- Any other game balance or mechanics

Always update the relevant design document section to match the code changes. This ensures the design documentation remains accurate and serves as a reliable reference.

### Key Files

- `projects/ffh-simplified/src/data/soldiers.ts` - Unit definitions
- `projects/ffh-simplified/src/data/buildings.ts` - Building definitions
- `projects/ffh-simplified/src/data/terrainFeatures.ts` - Terrain feature definitions
- `projects/ffh-simplified/src/game/Game.ts` - Core game logic
- `projects/ffh-simplified/src/game/Combat.ts` - Combat system
