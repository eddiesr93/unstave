(() => {
  'use strict'

  const links = [...document.querySelectorAll('[data-doc-link]')]
  const sections = [...document.querySelectorAll('[data-doc-section]')]
  const toast = document.querySelector('.copy-toast')
  let toastTimer

  function setActiveSection(id) {
    links.forEach((link) => {
      const active = link.hash === `#${id}`
      link.classList.toggle('is-active', active)
      if (active) link.setAttribute('aria-current', 'location')
      else link.removeAttribute('aria-current')
    })
  }

  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver((entries) => {
      const visible = entries
        .filter((entry) => entry.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)
      if (visible[0]) setActiveSection(visible[0].target.id)
    }, { rootMargin: '-15% 0px -70% 0px', threshold: 0 })

    sections.forEach((section) => observer.observe(section))
  }

  document.querySelectorAll('.docs-mobile-nav a').forEach((link) => {
    link.addEventListener('click', () => link.closest('details').removeAttribute('open'))
  })

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

  document.querySelectorAll('[data-pm-group]').forEach((group) => {
    const tabs = [...group.querySelectorAll('[data-pm-tab]')]
    const panels = [...group.querySelectorAll('[data-pm-panel]')]
    const copyButton = group.querySelector('[data-copy-target]')

    function selectManager(tab) {
      tabs.forEach((button) => {
        const active = button === tab
        button.setAttribute('aria-selected', String(active))
        button.tabIndex = active ? 0 : -1
      })
      panels.forEach((panel) => {
        panel.hidden = panel.dataset.pmPanel !== tab.dataset.pmTab
      })
      const active = panels.find((panel) => !panel.hidden)
      if (copyButton && active?.firstElementChild) {
        copyButton.dataset.copyTarget = active.firstElementChild.id
      }
    }

    tabs.forEach((tab, index) => {
      tab.tabIndex = tab.getAttribute('aria-selected') === 'true' ? 0 : -1
      tab.addEventListener('click', () => selectManager(tab))
      tab.addEventListener('keydown', (event) => {
        if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
        event.preventDefault()
        const direction = event.key === 'ArrowRight' ? 1 : -1
        const next = tabs[(index + direction + tabs.length) % tabs.length]
        selectManager(next)
        next.focus()
      })
    })
  })

  document.querySelectorAll('[data-copy-target]').forEach((button) => {
    button.addEventListener('click', async () => {
      const source = document.getElementById(button.dataset.copyTarget)
      if (!source) return

      try {
        await copyText(source.textContent)
        toast.textContent = 'Command copied'
      } catch {
        toast.textContent = 'Copy failed. Select the command manually.'
      }

      toast.classList.add('is-visible')
      clearTimeout(toastTimer)
      toastTimer = setTimeout(() => toast.classList.remove('is-visible'), 2200)
    })
  })
})()
