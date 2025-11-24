/*
 * WHAT IS THIS FILE?
 *
 * It's the entry point for the Fastify server when building for production.
 *
 * Learn more about Node.js server integrations here:
 * - https://qwik.dev/docs/deployments/node/
 *
 */
import { type PlatformNode } from "@builder.io/qwik-city/middleware/node";
import "dotenv/config";
import Fastify from "fastify";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import FastifyQwik from "./plugins/fastify-qwik";

declare global {
  type QwikCityPlatform = PlatformNode;
}

// Directories where the static assets are located
const distDir = join(fileURLToPath(import.meta.url), "..", "..", "dist");
const buildDir = join(distDir, "build");
const assetsDir = join(distDir, "assets");

// Allow for dynamic port and host
const PORT = parseInt(process.env.PORT ?? "3000");
const HOST = process.env.HOST ?? "0.0.0.0";

const start = async () => {
  // Create the fastify server
  // https://fastify.dev/docs/latest/Guides/Getting-Started/
  const fastify = Fastify({
    logger: true,
  });

  // Enable compression
  // https://github.com/fastify/fastify-compress
  // IMPORTANT NOTE: THIS MUST BE REGISTERED BEFORE THE fastify-qwik PLUGIN
  // await fastify.register(import('@fastify/compress'))

  // Proxy API requests to backend
  const BACKEND_URL = process.env.BACKEND_URL ?? "http://backend:3000";
  
  fastify.register(async function (fastify) {
    fastify.all('/api/*', async (request, reply) => {
      const backendUrl = `${BACKEND_URL}${request.url}`;
      console.log(`Proxying ${request.method} ${request.url} to ${backendUrl}`);
      
      try {
        const response = await fetch(backendUrl, {
          method: request.method,
          headers: {
            'Content-Type': 'application/json'
          },
          body: request.method !== 'GET' ? JSON.stringify(request.body) : undefined
        });
        
        const data = await response.json();
        reply.code(response.status).send(data);
      } catch (error) {
        console.error('Proxy error:', error);
        reply.code(500).send({ error: 'Backend unavailable' });
      }
    });
  });

  // Handle Qwik City using a plugin
  await fastify.register(FastifyQwik, { distDir, buildDir, assetsDir });

  // Start the fastify server
  await fastify.listen({ port: PORT, host: HOST });
};

start();
