// Weather Card Component
// Displays current weather conditions

(function() {
  'use strict';

  const WeatherCard = {
    name: 'weather-card',
    displayName: 'Weather Card',
    description: 'Display current weather conditions',

    defaults: {
      city: 'Shanghai',
      refreshInterval: 300000, // 5 minutes
    },

    async fetchData(extensionId, params) {
      const response = await fetch(`/api/extensions/${extensionId}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command: 'get_weather',
          params: { city: params.city || this.defaults.city }
        })
      });

      if (!response.ok) {
        throw new Error(`Failed to fetch weather: ${response.status}`);
      }

      return response.json();
    },

    render(container, params) {
      const city = params.city || this.defaults.city;

      container.innerHTML = `
        <div class="weather-card p-4 rounded-lg border bg-card">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-lg font-semibold">Weather</h3>
            <span class="text-sm text-muted-foreground">${city}</span>
          </div>
          <div class="weather-content text-center py-4">
            <div class="animate-pulse">
              <div class="h-8 bg-muted rounded w-24 mx-auto mb-2"></div>
              <div class="h-4 bg-muted rounded w-16 mx-auto"></div>
            </div>
          </div>
        </div>
      `;

      this.updateData(container, params);
    },

    async updateData(container, params) {
      const contentEl = container.querySelector('.weather-content');
      const extensionId = params.extensionId || 'neomind.weather.forecast';
      const city = params.city || this.defaults.city;

      try {
        const data = await this.fetchData(extensionId, { city });

        contentEl.innerHTML = `
          <div class="text-4xl font-bold mb-1">${Math.round(data.temperature_c)}°C</div>
          <div class="text-sm text-muted-foreground mb-2">${data.description}</div>
          <div class="flex justify-center gap-4 text-xs text-muted-foreground">
            <span>💧 ${data.humidity_percent}%</span>
            <span>💨 ${data.wind_speed_kmph} km/h</span>
          </div>
        `;
      } catch (error) {
        contentEl.innerHTML = `
          <div class="text-destructive text-sm">
            Failed to load weather data
          </div>
        `;
        console.error('Weather card error:', error);
      }
    }
  };

  // Register component globally
  if (typeof window !== 'undefined') {
    window.NeoMindExtensions = window.NeoMindExtensions || {};
    window.NeoMindExtensions['weather-card'] = WeatherCard;
  }

  // Export for module systems
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = WeatherCard;
  }
})();
