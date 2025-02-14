"use client";

import React, { useEffect, useState } from "react";
import { convert_utc_to_local } from "@/app/lib/utils";

interface WeatherDaily {
  time: string;
  temperature_2m_max: number;
  temperature_2m_min: number;
  precipitation_probability_max: number;
  weather_code: number;
}

interface Weather {
  time: string;
  temperature: number;
  weathercode: number;
  daily: WeatherDaily[];
}

interface WeatherData {
  name: string;
  latitude: number;
  longitude: number;
  weather: Weather;
}

const weatherCodeMap: Record<number, string> = {
  0: "☀️ 晴天",
  1: "🌤 多雲時晴",
  2: "⛅ 局部多雲",
  3: "☁️ 陰天",
  45: "🌫 有霧",
  48: "🌫 凍霧",
  51: "🌧 毛毛雨（小雨）",
  53: "🌧 毛毛雨（中雨）",
  55: "🌧 毛毛雨（大雨）",
  56: "🌨 凍雨（小雨）",
  57: "🌨 凍雨（大雨）",
  61: "🌧 小雨",
  63: "🌧 中雨",
  65: "🌧 大雨",
  66: "❄️ 凍雨（小）",
  67: "❄️ 凍雨（大）",
  71: "❄️ 小雪",
  73: "❄️ 中雪",
  75: "❄️ 大雪",
  77: "🌨 雪粒",
  80: "🌦 小陣雨",
  81: "🌦 中陣雨",
  82: "🌦 強陣雨",
  85: "❄️ 小陣雪",
  86: "❄️ 大陣雪",
  95: "⛈ 雷雨",
  96: "⛈ 雷雨夾小冰雹",
  99: "⛈ 雷雨夾大冰雹",
};

export default function Page() {
  const [weatherData, setWeatherData] = useState<WeatherData[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [expandedCity, setExpandedCity] = useState<string | null>(null);

  useEffect(() => {
    async function sendPostRequest() {
      try {
        const response = await fetch("/api/v1/cmd", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ cmd: "p weather show" }),
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        setWeatherData(data);
      } catch (err: any) {
        setError(err.message || "發生錯誤");
      }
    }

    sendPostRequest();
  }, []);

  // 根據系統語系格式化日期顯示 "YYYY-MM-DD (當地語系的星期)"
  const formatDateWithWeekday = (dateStr: string): string => {
    const date = new Date(dateStr);
    const locale = navigator.language || "en-US"; // 取得使用者語系
    const weekday = new Intl.DateTimeFormat(locale, {
      weekday: "short",
    }).format(date);
    return `${dateStr} (${weekday})`;
  };

  return (
    <div className="min-h-screen bg-gray-100 p-6">
      <h1 className="text-3xl font-bold text-center mb-6">🌍 天氣資訊</h1>
      <div className="overflow-x-auto">
        <table className="min-w-full bg-white shadow-md rounded-lg overflow-hidden">
          <thead className="bg-blue-500 text-white">
            <tr>
              <th className="py-3 px-6 text-left">📍 地點</th>
              <th className="py-3 px-6 text-left">📅 時間</th>
              <th className="py-3 px-6 text-left">🌡 溫度 (°C)</th>
              <th className="py-3 px-6 text-left">🌦 天氣</th>
              <th className="py-3 px-6 text-left">🔽 預報</th>
            </tr>
          </thead>
          <tbody>
            {weatherData.map((item, index) => (
              <React.Fragment key={item.name}>
                <tr
                  className={`border-b ${
                    index % 2 === 0 ? "bg-gray-50" : "bg-white"
                  }`}
                >
                  <td className="py-3 px-6">{item.name}</td>
                  <td className="py-3 px-6">
                    {item.weather.time
                      ? convert_utc_to_local(item.weather.time)
                      : "n/a"}
                  </td>
                  <td className="py-3 px-6">
                    {item.weather.temperature !== null
                      ? `${item.weather.temperature.toFixed(1)} °C`
                      : "n/a"}
                  </td>{" "}
                  <td className="py-3 px-6">
                    {weatherCodeMap[item.weather.weathercode] || "❓ 未知天氣"}
                  </td>
                  <td className="py-3 px-6">
                    <button
                      className="bg-blue-500 text-white px-4 py-2 rounded-lg"
                      onClick={() =>
                        setExpandedCity(
                          expandedCity === item.name ? null : item.name
                        )
                      }
                    >
                      {expandedCity === item.name ? "收合" : "展開"}
                    </button>
                  </td>
                </tr>

                {/* 展開的七天天氣預報 - 忽略第一天 */}
                {expandedCity === item.name && (
                  <tr>
                    <td colSpan={5} className="p-4 bg-gray-100">
                      <table className="w-full border-collapse">
                        <thead>
                          <tr className="bg-gray-300">
                            <th className="py-2 px-4 text-left">📆 日期</th>
                            <th className="py-2 px-4 text-left">🌦 天氣概況</th>
                            <th className="py-2 px-4 text-left">
                              ☔ 降雨機率 (%)
                            </th>
                            <th className="py-2 px-4 text-left">
                              🌡 高 / 低 溫 (°C)
                            </th>
                          </tr>
                        </thead>
                        <tbody>
                          {item.weather.daily.slice(1).map(
                            (
                              day,
                              i // 忽略第一天
                            ) => (
                              <tr key={i} className="border-b border-gray-300">
                                <td className="py-2 px-4">
                                  {formatDateWithWeekday(day.time)}
                                </td>
                                <td className="py-2 px-4">
                                  {weatherCodeMap[day.weather_code] ||
                                    "❓ 未知天氣"}
                                </td>
                                <td className="py-2 px-4">
                                  {day.precipitation_probability_max}%
                                </td>
                                <td className="py-2 px-4">
                                  {day.temperature_2m_max.toFixed(1)}°C /{" "}
                                  {day.temperature_2m_min.toFixed(1)}°C
                                </td>
                              </tr>
                            )
                          )}
                        </tbody>
                      </table>
                    </td>
                  </tr>
                )}
              </React.Fragment>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
