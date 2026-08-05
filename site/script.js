(() => {
  'use strict'

  const nodeDetails = {
    main: {
      kind: 'entry module',
      path: 'src/main.tsx',
      description: 'Imports one symbol from src/a/index.ts. The source reads as one edge; runtime reachability expands behind it.',
      metrics: [['Imports', '1 barrel'], ['Uses', '1 symbol'], ['Reachable', '6 modules']],
      note: 'Importer at the start of the amplification path.',
    },
    'barrel-a': {
      kind: 'pure barrel',
      path: 'src/a/index.ts',
      description: 'The importer asks for one symbol. This barrel opens a six-module re-export closure.',
      metrics: [['Actual cost', '6 modules'], ['Excess', '5 modules'], ['Rewrite', 'safe']],
      note: 'The highest-impact imported barrel in this fixture.',
    },
    'barrel-b': {
      kind: 'nested barrel',
      path: 'src/a/b/index.ts',
      description: 'A second re-export hub inside the closure. Nested barrels compound the amount of graph pulled through the original import.',
      metrics: [['Re-exports', '2 paths'], ['Own declarations', '0'], ['Side effects', 'none']],
      note: 'Not imported directly, but part of the measured transitive cost.',
    },
    'barrel-c': {
      kind: 'nested barrel',
      path: 'src/a/b/c/index.ts',
      description: 'The final re-export layer before the declaration used by the importer.',
      metrics: [['Re-exports', '1 path'], ['Own declarations', '0'], ['Side effects', 'none']],
      note: 'Re-export chains are followed until a unique declaration is proven.',
    },
    one: {
      kind: 'declaration / rewrite target',
      path: 'src/a/b/c/one.ts',
      description: 'The symbol resolves uniquely here. unstave can replace the barrel import with this direct path.',
      metrics: [['Declares', '1 symbol'], ['Ambiguity', 'none'], ['Rewrite', 'direct']],
      note: "Result: import { one } from './a/b/c/one'",
    },
    two: {
      kind: 'unused declaration',
      path: 'src/a/b/c/two.ts',
      description: 'Reachable through the barrel chain, but not requested by the original importer.',
      metrics: [['Requested', 'no'], ['Graph cost', '+1 module'], ['After rewrite', 'excluded']],
      note: 'One of five excess modules removed from this import path.',
    },
    three: {
      kind: 'unused declaration',
      path: 'src/a/b/c/three.ts',
      description: 'Another declaration loaded only because the barrel exposes it through the same public surface.',
      metrics: [['Requested', 'no'], ['Graph cost', '+1 module'], ['After rewrite', 'excluded']],
      note: 'Barrel cost is measured from reachability, not file size.',
    },
  }

  const viewDetails = {
    cost: 'barrel-a',
    topology: 'main',
    rewrite: 'one',
  }

  const inspectorKind = document.querySelector('#inspector-kind')
  const inspectorPath = document.querySelector('#inspector-path')
  const inspectorDescription = document.querySelector('#inspector-description')
  const inspectorMetrics = document.querySelector('#inspector-metrics')
  const inspectorNote = document.querySelector('#inspector-note')
  const graphStage = document.querySelector('.graph-stage')
  const graphNodes = [...document.querySelectorAll('[data-node]')]

  function selectNode(id) {
    const details = nodeDetails[id]
    if (!details) return

    graphNodes.forEach((node) => node.classList.toggle('is-selected', node.dataset.node === id))
    inspectorKind.textContent = details.kind
    inspectorPath.textContent = details.path
    inspectorDescription.textContent = details.description
    inspectorNote.textContent = details.note
    inspectorMetrics.replaceChildren()

    details.metrics.forEach(([label, value]) => {
      const group = document.createElement('div')
      const term = document.createElement('dt')
      const description = document.createElement('dd')
      term.textContent = label
      description.textContent = value
      group.append(term, description)
      inspectorMetrics.append(group)
    })
  }

  graphNodes.forEach((node) => {
    node.addEventListener('click', () => selectNode(node.dataset.node))
    node.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        selectNode(node.dataset.node)
      }
    })
  })

  document.querySelectorAll('[data-view]').forEach((button) => {
    button.addEventListener('click', () => {
      const view = button.dataset.view
      graphStage.dataset.graphView = view
      document.querySelectorAll('[data-view]').forEach((candidate) => {
        const active = candidate === button
        candidate.classList.toggle('is-active', active)
        candidate.setAttribute('aria-pressed', String(active))
      })
      selectNode(viewDetails[view])
    })
  })

  const toast = document.querySelector('.copy-toast')
  let toastTimer

  async function copyText(value) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value)
      return
    }

    const input = document.createElement('textarea')
    input.value = value
    input.setAttribute('readonly', '')
    input.style.position = 'fixed'
    input.style.opacity = '0'
    document.body.append(input)
    input.select()
    document.execCommand('copy')
    input.remove()
  }

  document.querySelectorAll('[data-copy]').forEach((button) => {
    button.addEventListener('click', async () => {
      try {
        await copyText(button.dataset.copy)
        toast.textContent = 'Command copied'
      } catch {
        toast.textContent = 'Copy failed — select the command manually'
      }
      toast.classList.add('is-visible')
      clearTimeout(toastTimer)
      toastTimer = setTimeout(() => toast.classList.remove('is-visible'), 2200)
    })
  })

  const installTabs = [...document.querySelectorAll('[data-install-tab]')]
  const installPanels = [...document.querySelectorAll('.install-content[role="tabpanel"]')]

  function selectInstallTab(tab) {
    installTabs.forEach((button) => {
      const active = button === tab
      button.setAttribute('aria-selected', String(active))
      button.tabIndex = active ? 0 : -1
    })
    installPanels.forEach((panel) => {
      panel.hidden = panel.id !== `install-${tab.dataset.installTab}`
    })
  }

  installTabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectInstallTab(tab))
    tab.addEventListener('keydown', (event) => {
      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
      event.preventDefault()
      const direction = event.key === 'ArrowRight' ? 1 : -1
      const next = installTabs[(index + direction + installTabs.length) % installTabs.length]
      selectInstallTab(next)
      next.focus()
    })
  })

  const reveals = [...document.querySelectorAll('.reveal')]
  if ('IntersectionObserver' in window && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return
        entry.target.classList.add('is-visible')
        observer.unobserve(entry.target)
      })
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 })

    reveals.forEach((element) => observer.observe(element))
  } else {
    reveals.forEach((element) => element.classList.add('is-visible'))
  }
})()
