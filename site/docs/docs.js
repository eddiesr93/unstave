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
