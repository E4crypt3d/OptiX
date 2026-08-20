/**
 * Downsample a long time series for charting using the Largest-Triangle-
 * Three-Buckets (LTTB) algorithm. LTTB preserves the visual shape of the
 * data — including spikes (1% lows in frame-time data) — far better than
 * naive stride sampling, in O(n).
 *
 * A full benchmark capture can contain tens of thousands of frame times;
 * rendering every point as an SVG node freezes the UI. `maxPoints` caps the
 * chart data while retaining the overall trend.
 */

export interface DecimatablePoint {
  frame: number;
  ms: number;
}

export function decimateFrameTimes(
  data: DecimatablePoint[],
  maxPoints: number,
): DecimatablePoint[] {
  const n = data.length;
  if (n <= maxPoints || maxPoints < 3) return data;

  // Bucket size, leaving room for the first and last points.
  const every = (n - 2) / (maxPoints - 2);

  const sampled: DecimatablePoint[] = [data[0]];
  let a = 0; // index of the previous "anchor" point

  for (let i = 0; i < maxPoints - 2; i++) {
    // Average of the next bucket (the "c" candidates).
    let avgStart = Math.floor((i + 1) * every) + 1;
    let avgEnd = Math.floor((i + 2) * every) + 1;
    if (avgEnd > n) avgEnd = n;
    const avgLen = avgEnd - avgStart;
    let avgX = 0;
    let avgY = 0;
    for (let j = avgStart; j < avgEnd; j++) {
      avgX += data[j].frame;
      avgY += data[j].ms;
    }
    avgX /= avgLen;
    avgY /= avgLen;

    // Pick the point in the current bucket forming the largest triangle
    // with the anchor and the next-bucket average.
    const rangeStart = Math.floor(i * every) + 1;
    const rangeEnd = Math.floor((i + 1) * every) + 1;
    const ax = data[a].frame;
    const ay = data[a].ms;
    let maxArea = -1;
    let best = a;
    for (let j = rangeStart; j < rangeEnd; j++) {
      // Twice the triangle area (the 0.5 factor is irrelevant for ranking).
      const area =
        Math.abs(
          (ax - avgX) * (data[j].ms - ay) - (ax - data[j].frame) * (avgY - ay),
        ) * 0.5;
      if (area > maxArea) {
        maxArea = area;
        best = j;
      }
    }
    sampled.push(data[best]);
    a = best;
  }

  sampled.push(data[n - 1]);
  return sampled;
}