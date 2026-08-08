#!/usr/bin/env node
// Post the unstave launch to Reddit by driving your already-logged-in Chrome
// over the DevTools protocol (no API app, no credentials in the terminal).
//
// Prerequisites:
//   1. Fully quit Chrome, then relaunch it with the debugging port open:
//        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
//          --remote-debugging-port=9222
//      (Linux/Windows: find your Chrome binary and pass the same flag.)
//   2. In that Chrome window, log in to reddit.com.
//   3. Run:
//        node docs/launch/post-reddit-chrome.mjs
//
// Behaviour: opens a tab per post, fills title + body, clicks submit. If
// Reddit shows a captcha, a required flair dialog, or anything unexpected, the
// script stops the loop, leaves the tab open, and prints what it saw so you can
// finish that one by hand. Nothing is ever posted without your login being the
// one submitting.

import { chromium } from 'playwright-core'

import { SUBREDDITS } from './reddit-posts.mjs'

const DEFAULT_SUBS = ['typescript', 'reactjs']
const CDP_URL = process.env.REDDIT_CDP_URL ?? 'http://localhost:9222'

const TITLE_SELECTORS = [
  'textarea#post-title',
  'textarea[aria-label*="title" i]',
  'input[aria-label*="title" i]',
  'textarea[placeholder*="title" i]',
  'input[placeholder*="title" i]',
]

const BODY_SELECTORS = [
  'div[role="textbox"][contenteditable="true"]',
  'div[contenteditable="true"]',
  'textarea[aria-label*="text" i]',
  'textarea[name="text"]',
]

const SUBMIT_SELECTORS = [
  'button[type="submit"]',
  'button[data-testid="submit"]',
  'button:has-text("Post")',
  'button:has-text("Submit")',
]

function parseSubs() {
  const args = process.argv.slice(2)
  const subs = []
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--sub' && args[i + 1]) {
      subs.push(args[i + 1])
      i++
    }
  }
  return subs.length > 0 ? subs : DEFAULT_SUBS
}

async function visibleFirst(page, selectors, timeout) {
  for (const selector of selectors) {
    const locator = page.locator(selector).first()
    try {
      await locator.waitFor({ state: 'visible', timeout })
      return locator
    } catch {
      // try the next candidate
    }
  }
  return null
}

async function dumpModals(page) {
  const modals = page.locator('[aria-modal="true"]:visible, [role="dialog"]:visible')
  const count = await modals.count()
  if (count > 0) {
    const text = await modals.first().innerText().catch(() => '')
    console.log(`  ⚠ an inline dialog appeared (${count}):\n${text.slice(0, 600)}`)
    return true
  }
  return false
}

async function postOne(page, sub, { url, title, text }) {
  console.log(`\n▶ ${sub}`)
  await page.goto(`${url}submit?type=TEXT`, { waitUntil: 'domcontentloaded', timeout: 45000 })

  const redirected = new URL(page.url()).pathname
  if (redirected.includes('/login')) {
    console.log(`  ✗ redirected to login — log into Reddit in this Chrome first, then re-run.`)
    return false
  }
  if (new URL(page.url()).searchParams.get('type') !== 'TEXT' && redirected.includes('submit') === false) {
    console.log(`  ⚠ landed on ${redirected} instead of the submit form.`)
  }

  const titleBox = await visibleFirst(page, TITLE_SELECTORS, 25000)
  if (!titleBox) {
    console.log('  ⚠ could not find the title field. Leaving the tab open for manual finishing.')
    return false
  }
  await titleBox.click()
  await titleBox.fill(title)

  let bodyBox = await visibleFirst(page, BODY_SELECTORS, 15000)
  if (!bodyBox) {
    console.log('  ⚠ could not find the body editor. Leaving the tab open for manual finishing.')
    return false
  }
  const tag = (await bodyBox.evaluate((el) => el.tagName)) ?? 'div'
  if (tag.toLowerCase() === 'textarea') {
    await bodyBox.fill(text)
  } else {
    // contenteditable rich text editor: insert text through the keyboard
    await bodyBox.click()
    await page.keyboard.insertText(text)
  }

  // let any on-screen validation settle, then submit
  await page.waitForTimeout(1200)
  await dumpModals(page)

  const submit = await visibleFirst(page, SUBMIT_SELECTORS, 10000)
  if (!submit) {
    console.log('  ⚠ could not find the submit button. Leaving the tab open for manual finishing.')
    return false
  }
  await submit.click()

  // confirmation: reddit lands on /comments/<id> for a live post
  const confirmed = await page
    .waitForURL('**/comments/**', { timeout: 30000 })
    .then(() => true)
    .catch(() => false)

  if (confirmed) {
    const finalUrl = page.url()
    const id = finalUrl.match(/comments\/([a-z0-9]+)/)?.[1]
    console.log(`  ✓ posted: ${finalUrl}${id ? ` (id ${id})` : ''}`)
    return true
  }

  const hasModal = await dumpModals(page)
  console.log(
    hasModal
      ? '  ⚠ a dialog stopped the submit (likely flair or captcha). Finish it in the open tab, then I will continue to the next post.'
      : '  ⚠ could not confirm the post went live. Check the open tab — finish it by hand if needed.',
  )
  return false
}

async function main() {
  const subs = parseSubs()
  console.log(`Connecting to Chrome at ${CDP_URL} …`)
  const browser = await chromium.connectOverCDP(CDP_URL)
  const contexts = browser.contexts()
  if (contexts.length === 0) {
    console.error('✗ no browser context found; open a tab in Chrome first.')
    await browser.close()
    process.exit(1)
  }
  const context = contexts[0]
  console.log(`Connected. Posting to: ${subs.join(', ')} (your logged-in account)`)

  for (const sub of subs) {
    if (!SUBREDDITS[sub]) {
      console.error(`  unknown sub '${sub}' — allowed: ${Object.keys(SUBREDDITS).join(', ')}`)
      continue
    }
    const page = await context.newPage()
    try {
      const ok = await postOne(page, sub, SUBREDDITS[sub])
      if (!ok) {
        console.log(`  → stopping the loop so you can finish ${sub} manually; re-run to continue.`)
        break
      }
    } finally {
      if (page.isClosed() === false) {
        // keep the tab open for inspection if something needs a human
        console.log(`  (tab left open: ${page.url()})`)
      }
    }
    if (sub !== subs[subs.length - 1]) {
      await new Promise((resolve) => setTimeout(resolve, 90000))
    }
  }

  console.log('\nDone. Close Chrome terminal afterwards: Ctrl+C on the Chrome launcher.')
  await browser.close().catch(() => {})
}

main().catch((error) => {
  console.error(`\n✗ driver error: ${error.message}`)
  console.error('Is Chrome running with --remote-debugging-port=9222?')
  process.exit(1)
})
