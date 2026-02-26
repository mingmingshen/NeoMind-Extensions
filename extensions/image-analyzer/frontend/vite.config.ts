import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [
    react({
      // Use classic JSX runtime to avoid jsx-runtime dependency
      jsxRuntime: 'classic'
    })
  ],
  define: {
    // Replace process.env.NODE_ENV with a string literal to avoid "process is not defined" error
    'process.env.NODE_ENV': JSON.stringify('production')
  },
  build: {
    lib: {
      entry: './src/index.tsx',
      name: 'ImageAnalyzerDashboardComponents',
      fileName: () => `image-analyzer-components.js`,
      formats: ['iife']  // IIFE assigns to global variable
    },
    rollupOptions: {
      // Externalize React to use the host app's React
      external: ['react', 'react-dom'],
      output: {
        // Use 'named' exports to make named exports available at global[name].ExportName
        exports: 'named',
        // Provide global variables for external dependencies
        globals: {
          'react': 'React',
          'react-dom': 'ReactDOM'
        }
      }
    }
  }
})
