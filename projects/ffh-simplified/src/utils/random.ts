// Seeded random number generator (mulberry32)
export function createRNG(seed: number): () => number {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function randomInt(rng: () => number, min: number, max: number): number {
  return Math.floor(rng() * (max - min + 1)) + min;
}

export function randomElement<T>(rng: () => number, array: T[]): T {
  return array[Math.floor(rng() * array.length)];
}

export function shuffleArray<T>(rng: () => number, array: T[]): T[] {
  const result = [...array];
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

// Generate a unique ID
let idCounter = 0;
export function generateId(prefix: string = 'id'): string {
  return `${prefix}_${++idCounter}_${Date.now().toString(36)}`;
}

// Fantasy first names for soldiers
const SOLDIER_NAMES = [
  // Human-style names
  'Marcus', 'Elena', 'Aldric', 'Sera', 'Gareth', 'Lyra', 'Theron', 'Vera',
  'Caspian', 'Nadia', 'Roland', 'Mira', 'Cedric', 'Freya', 'Godric', 'Isolde',
  'Brennan', 'Astrid', 'Corwin', 'Dahlia', 'Edwin', 'Gwen', 'Hugo', 'Ingrid',
  'Jasper', 'Kira', 'Leander', 'Morgana', 'Nolan', 'Ophelia', 'Pierce', 'Quinn',
  'Roderick', 'Sabine', 'Tristan', 'Una', 'Victor', 'Wren', 'Xavier', 'Yvaine',
  // More exotic names
  'Aelric', 'Brynn', 'Caelum', 'Darian', 'Elara', 'Fenris', 'Gideon', 'Helena',
  'Ivar', 'Jorah', 'Kaelen', 'Lucian', 'Magnus', 'Nerys', 'Orin', 'Petra',
  'Quillan', 'Raven', 'Soren', 'Talia', 'Ulric', 'Vex', 'Wyatt', 'Zara',
  'Alaric', 'Briar', 'Cora', 'Drake', 'Ember', 'Flynn', 'Greta', 'Hadrian',
];

// Generate a random soldier name
export function generateSoldierName(): string {
  const index = Math.floor(Math.random() * SOLDIER_NAMES.length);
  return SOLDIER_NAMES[index];
}

// Generate a random soldier name using a seeded RNG
export function generateSoldierNameSeeded(rng: () => number): string {
  return randomElement(rng, SOLDIER_NAMES);
}

// Fantasy city names
const CITY_NAMES = [
  // Classic fantasy cities
  'Ironhold', 'Silvermere', 'Ravenspire', 'Thornwood', 'Stormhaven',
  'Ashford', 'Goldcrest', 'Shadowfen', 'Wintervale', 'Brightwater',
  'Dragonfall', 'Moonreach', 'Sunspire', 'Darkholm', 'Frostgate',
  'Crystalbrook', 'Emberkeep', 'Starfall', 'Windmere', 'Stonehearth',
  // More varied names
  'Verdantis', 'Kaldris', 'Thorngate', 'Valorheim', 'Mistwood',
  'Duskhollow', 'Ironforge', 'Highcliff', 'Deepwell', 'Redmarsh',
  'Whiteshore', 'Blackmoor', 'Greenvale', 'Oldbridge', 'Kingshaven',
  'Queensport', 'Eaglecrest', 'Wolfden', 'Lionheart', 'Hawkridge',
  // Ancient-sounding names
  'Aethoria', 'Valdros', 'Kaelindor', 'Mythros', 'Seraphel',
  'Amaranth', 'Celestine', 'Dawnhold', 'Eventide', 'Faerhold',
];

// Track used city names to avoid duplicates
let usedCityNames: Set<string> = new Set();

// Reset used city names (call when starting a new game)
export function resetCityNames(): void {
  usedCityNames = new Set();
}

// Generate a random city name
export function generateCityName(): string {
  // Find an unused name
  const availableNames = CITY_NAMES.filter(name => !usedCityNames.has(name));

  if (availableNames.length === 0) {
    // All names used, generate a numbered fallback
    const index = usedCityNames.size - CITY_NAMES.length + 1;
    return `Settlement ${index}`;
  }

  const index = Math.floor(Math.random() * availableNames.length);
  const name = availableNames[index];
  usedCityNames.add(name);
  return name;
}

// Generate a random city name using a seeded RNG
export function generateCityNameSeeded(rng: () => number): string {
  const availableNames = CITY_NAMES.filter(name => !usedCityNames.has(name));

  if (availableNames.length === 0) {
    const index = usedCityNames.size - CITY_NAMES.length + 1;
    return `Settlement ${index}`;
  }

  const name = randomElement(rng, availableNames);
  usedCityNames.add(name);
  return name;
}
