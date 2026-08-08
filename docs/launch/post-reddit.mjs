#!/usr/bin/env node
// Post the unstave launch to selected Reddit subreddits via the official API.
//
// Setup (2 minutes, once):
//   1. Create a "script" app at https://www.reddit.com/prefs/apps
//      (name anything, type "script", any redirect URI e.g. http://localhost)
//      -> note the client ID (under the app name) and secret.
//   2. Export credentials and run:
//
//      export REDDIT_CLIENT_ID=...
//      export REDDIT_CLIENT_SECRET=...
//      export REDDIT_USERNAME=...
//      export REDDIT_PASSWORD=...
//      node docs/launch/post-reddit.mjs              # r/typescript + r/reactjs
//      node docs/launch/post-reddit.mjs --sub r/typescript
//
// Credentials are read only from the environment; nothing is logged.
//
// Note: new accounts / heavy links can be auto-removed by subreddit spam
// filters (r/programming and r/rust are strictest). If a post is removed, it
// may need a mod approve; results print each submission URL.

import { SUBREDDITS } from './reddit-posts.mjs'

const DEFAULTS = ['typescript', 'reactjs']

function parseArgs() {
  const args = process.argv.slice(2)
  const subs = []
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--sub' && args[i + 1]) {
      subs.push(args[i + 1])
      i++
    }
  }
  return subs.length > 0 ? subs : DEFAULTS
}

function credentials() {
  const keys = ['REDDIT_CLIENT_ID', 'REDDIT_CLIENT_SECRET', 'REDDIT_USERNAME', 'REDDIT_PASSWORD']
  for (const key of keys) {
    if (!process.env[key]) {
      console.error(`missing ${key} - set it first (see header of this script)`)
      process.exit(2)
    }
  }
  return keys.map((key) => process.env[key])
}

async function accessToken() {
  const [clientId, clientSecret, username, password] = credentials()
  const body = new URLSearchParams({
    grant_type: 'password',
    username,
    password,
    scope: '*',
  })
  const response = await fetch('https://www.reddit.com/api/v1/access_token', {
    method: 'POST',
    headers: {
      Authorization: `Basic ${Buffer.from(`${clientId}:${clientSecret}`).toString('base64')}`,
      'Content-Type': 'application/x-www-form-urlencoded',
      'User-Agent': 'unstave-launch/0.2.2 (by /u/' + username + ')',
    },
    body,
  })
  const data = await response.json()
  if (!data.access_token) {
    console.error(`OAuth failed (${response.status}): ${JSON.stringify(data)}`)
    process.exit(1)
  }
  return data.access_token
}

async function submit(token, sub, { title, text }) {
  const body = new URLSearchParams({
    api_type: 'json',
    kind: 'self',
    sr: `r/${sub}`,
    title,
    text,
    resubmit: 'true',
  })
  const response = await fetch('https://oauth.reddit.com/api/submit', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/x-www-form-urlencoded',
      'User-Agent': 'unstave-launch/0.2.2',
    },
    body,
  })
  const data = await response.json()
  const errors = data?.json?.errors ?? []
  if (errors.length > 0) {
    console.log(`[${sub}] errors: ${errors.map((e) => e.join(': ')).join('; ')}`)
    return
  }
  const url = data?.json?.data?.url ?? 'submitted'
  console.log(`[${sub}] posted: ${url}`)
}

async function main() {
  const subs = parseArgs()
  const token = await accessToken()
  console.log(`Posting to: ${subs.join(', ')}`)
  for (const sub of subs) {
    if (!SUBREDDITS[sub]) {
      console.error(`unknown sub '${sub}' - allowed: ${Object.keys(SUBREDDITS).join(', ')}`)
      continue
    }
    await submit(token, sub, SUBREDDITS[sub])
    await new Promise((resolve) => setTimeout(resolve, 8000))
  }
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
