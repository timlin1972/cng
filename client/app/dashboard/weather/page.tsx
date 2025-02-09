"use client";

import { useEffect, useState } from "react";

interface WeatherData {
  name: string;
  latitude: number;
  longitude: number;
  ts: number;
  temperature: number;
  code: number;
}

// 天氣代碼對應表
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

  useEffect(() => {
    async function sendPostRequest() {
      try {
        const response = await fetch("/api/v1/cmd", {
          method: "POST", // 指定使用 POST 方法
          headers: {
            "Content-Type": "application/json", // 告知伺服器資料格式
          },
          body: JSON.stringify({ cmd: "p weather show" }), // 請求主體資料
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

    // 當元件掛載時自動呼叫 POST 請求
    sendPostRequest();
  }, []); // 空的依賴陣列表示只在元件首次掛載時執行

  const formatTimestamp = (ts: number) => {
    const date = new Date(ts * 1000);
    return date.toLocaleString("zh-TW", { hour12: false });
  };

  return (
    <div className="min-h-screen bg-gray-100 p-6">
      <h1 className="text-3xl font-bold text-center mb-6">🌍 天氣資訊</h1>
      <div className="overflow-x-auto">
        <table className="min-w-full bg-white shadow-md rounded-lg overflow-hidden">
          {/* 表頭 */}
          <thead className="bg-blue-500 text-white">
            <tr>
              <th className="py-3 px-6 text-left">📍 地點</th>
              <th className="py-3 px-6 text-left">📅 時間</th>
              <th className="py-3 px-6 text-left">🌡 溫度 (°C)</th>
              <th className="py-3 px-6 text-left">🌦 天氣</th>
            </tr>
          </thead>
          {/* 表格內容 */}
          <tbody>
            {weatherData.map((item, index) => (
              <tr
                key={item.name}
                className={`border-b ${
                  index % 2 === 0 ? "bg-gray-50" : "bg-white"
                }`}
              >
                <td className="py-3 px-6">{item.name}</td>
                <td className="py-3 px-6">
                  {item.ts !== null ? formatTimestamp(item.ts) : "n/a"}
                </td>
                <td className="py-3 px-6">
                  {item.temperature !== null
                    ? item.temperature.toFixed(1)
                    : "n/a"}
                </td>
                <td className="py-3 px-6">
                  {item.code !== null
                    ? weatherCodeMap[item.code] || "❓ 未知天氣"
                    : "n/a"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
