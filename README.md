# Grammar Editor Beta 🚀✨

**Advanced AI-Powered Grammar Correction Editor**

A next-generation writing assistant built with Qwik and powered by T5 transformer models for intelligent grammar correction.

## 🎯 Beta Features

- **🤖 T5 AI Grammar Correction**: Advanced neural network-based grammar checking
- **⚡ Real-time Highlighting**: Instant visual feedback as you type
- **🎨 Smart Editor**: Built on Editor.js with custom grammar-aware plugins
- **🔧 Precise Corrections**: Character-level accuracy for highlighting and suggestions
- **🌐 Multi-dialect Support**: American, British, and Canadian English

---

## Project Structure

This project is using Qwik with [QwikCity](https://qwik.dev/qwikcity/overview/). QwikCity is just an extra set of tools on top of Qwik to make it easier to build a full site, including directory-based routing, layouts, and more.

Inside your project, you'll see the following directory structure:

```
├── public/
│   └── ...
└── src/
    ├── components/
    │   └── ...
    └── routes/
        └── ...
```

- `src/routes`: Provides the directory-based routing, which can include a hierarchy of `layout.tsx` layout files, and an `index.tsx` file as the page. Additionally, `index.ts` files are endpoints. Please see the [routing docs](https://qwik.dev/qwikcity/routing/overview/) for more info.

- `src/components`: Recommended directory for components.

- `public`: Any static assets, like images, can be placed in the public directory. Please see the [Vite public directory](https://vitejs.dev/guide/assets.html#the-public-directory) for more info.

## Add Integrations and deployment

Use the `npm run qwik add` command to add additional integrations. Some examples of integrations includes: Cloudflare, Netlify or Express Server, and the [Static Site Generator (SSG)](https://qwik.dev/qwikcity/guides/static-site-generation/).

```shell
npm run qwik add # or `yarn qwik add`
```

## 🚀 Quick Start (Beta)

### Prerequisites
- Node.js 18+ 
- Rust (for T5 backend)

### 1. Install Dependencies
```shell
pnpm install
```

### 2. Start Full Development Environment
```shell
pnpm run full-dev
```
This starts both the Rust T5 backend (port 3000) and Qwik frontend (port 5173) simultaneously.

### 3. Alternative: Manual Setup
```shell
# Terminal 1: Start T5 Grammar Backend
pnpm run backend

# Terminal 2: Start Frontend
pnpm run beta
```

### 4. Open Browser
Navigate to `http://localhost:5173` and start writing with AI-powered grammar correction!

## 🧪 Beta Testing

- **Real-time Grammar Checking**: Type in the editor and see instant T5-powered corrections
- **Smart Highlighting**: Click on highlighted errors to see AI suggestions
- **Multi-error Handling**: T5 can fix multiple grammar issues simultaneously
- **Natural Corrections**: Advanced sampling produces human-like corrections

> Note: The T5 model downloads automatically on first use (~500MB)

## Preview

The preview command will create a production build of the client modules, a production build of `src/entry.preview.tsx`, and run a local server. The preview server is only for convenience to preview a production build locally and should not be used as a production server.

```shell
npm run preview # or `yarn preview`
```

## Production

The production build will generate client and server modules by running both client and server build commands. The build command will use Typescript to run a type check on the source code.

```shell
npm run build # or `yarn build`
```
