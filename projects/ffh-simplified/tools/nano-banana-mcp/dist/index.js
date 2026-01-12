#!/usr/bin/env node
import 'dotenv/config';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { registerTools } from './server.js';
async function main() {
    const server = new McpServer({
        name: 'nano-banana-mcp',
        version: '1.0.0',
    });
    // Register all tools
    registerTools(server);
    // Connect via stdio transport
    const transport = new StdioServerTransport();
    await server.connect(transport);
    // Log to stderr (stdout is used for MCP communication)
    console.error('Nano Banana MCP server started');
}
main().catch((error) => {
    console.error('Failed to start server:', error);
    process.exit(1);
});
