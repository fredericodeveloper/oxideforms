/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./templates/**/*.html'],
  theme: {
    extend: {
      fontFamily: { sans: ['Inter', 'ui-sans-serif', 'system-ui', 'sans-serif'] },
      colors: {
        // Neutral surfaces, ordered by depth: page < card < raised (inputs).
        surface: { DEFAULT: '#0a0a0a', card: '#141414', raised: '#1e1e1e' },
        // The single accent colour: primary actions, links, focus, checked controls, the mark.
        accent: { DEFAULT: '#2563eb', hover: '#1d4ed8' },
      },
    },
  },
  plugins: [],
};
