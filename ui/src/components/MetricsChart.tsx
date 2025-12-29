import ReactECharts from "echarts-for-react";
import { useStats } from "../hooks/useQueries";
import { useEffect, useRef, useState } from "react";

export const MetricsChart = () => {
  const { data: stats } = useStats();
  const [chartData, setChartData] = useState<{
    timestamps: string[];
    received: number[];
    written: number[];
  }>({
    timestamps: [],
    received: [],
    written: [],
  });

  const dataPoints = useRef<{
    timestamps: string[];
    received: number[];
    written: number[];
  }>({
    timestamps: [],
    received: [],
    written: [],
  });

  useEffect(() => {
    if (stats) {
      const now = new Date().toLocaleTimeString();
      const data = dataPoints.current;

      data.timestamps.push(now);
      data.received.push(stats.records_received);
      data.written.push(stats.records_written);

      // Keep only last 30 data points
      if (data.timestamps.length > 30) {
        data.timestamps.shift();
        data.received.shift();
        data.written.shift();
      }

      setChartData({ ...data });
    }
  }, [stats]);

  const option = {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "axis",
      backgroundColor: "rgba(0, 0, 0, 0.8)",
      borderColor: "#60a5fa",
      borderWidth: 1,
      textStyle: {
        color: "#fff",
      },
    },
    legend: {
      data: ["Received", "Written"],
      textStyle: {
        color: "#e5e7eb",
      },
    },
    grid: {
      left: "3%",
      right: "4%",
      bottom: "3%",
      containLabel: true,
    },
    xAxis: {
      type: "category",
      boundaryGap: false,
      data: chartData.timestamps,
      axisLine: {
        lineStyle: {
          color: "#4b5563",
        },
      },
      axisLabel: {
        color: "#9ca3af",
      },
    },
    yAxis: {
      type: "value",
      axisLine: {
        lineStyle: {
          color: "#4b5563",
        },
      },
      axisLabel: {
        color: "#9ca3af",
      },
      splitLine: {
        lineStyle: {
          color: "#374151",
        },
      },
    },
    series: [
      {
        name: "Received",
        type: "line",
        smooth: true,
        data: chartData.received,
        lineStyle: {
          color: "#60a5fa",
          width: 2,
        },
        itemStyle: {
          color: "#60a5fa",
        },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: "rgba(96, 165, 250, 0.3)" },
              { offset: 1, color: "rgba(96, 165, 250, 0)" },
            ],
          },
        },
      },
      {
        name: "Written",
        type: "line",
        smooth: true,
        data: chartData.written,
        lineStyle: {
          color: "#34d399",
          width: 2,
        },
        itemStyle: {
          color: "#34d399",
        },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: "rgba(52, 211, 153, 0.3)" },
              { offset: 1, color: "rgba(52, 211, 153, 0)" },
            ],
          },
        },
      },
    ],
  };

  return (
    <div className="chart-container">
      <ReactECharts option={option} style={{ height: "300px" }} />
    </div>
  );
};
