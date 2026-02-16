/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"IBM Plex Sans"', 'system-ui', 'sans-serif'],
        mono: ['"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      boxShadow: {
        elev: 'var(--shadow-elev)',
        inset: 'var(--shadow-inset)',
      },
      borderRadius: {
        panel: 'var(--radius-panel)',
        control: 'var(--radius-control)',
      },
      colors: {
        env: {
          0: 'rgb(var(--env-0) / <alpha-value>)',
          1: 'rgb(var(--env-1) / <alpha-value>)',
        },
        plastic: {
          0: 'rgb(var(--plastic-0) / <alpha-value>)',
          1: 'rgb(var(--plastic-1) / <alpha-value>)',
        },
        ink: {
          0: 'rgb(var(--ink-0) / <alpha-value>)',
          1: 'rgb(var(--ink-1) / <alpha-value>)',
        },
        accent: {
          orange: 'rgb(var(--accent-orange) / <alpha-value>)',
          green: 'rgb(var(--accent-green) / <alpha-value>)',
          red: 'rgb(var(--accent-red) / <alpha-value>)',
          blue: 'rgb(var(--accent-blue) / <alpha-value>)',
        },
      },
    },
  },
  plugins: [],
}
