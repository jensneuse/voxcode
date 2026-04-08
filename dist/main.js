const { invoke, Channel } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const NUM_BARS = 24;
let bars = [];
let amplitudeChannel = null;
let amplitudeHistory = [];
const HISTORY_SIZE = 6; // smooth over recent values
let animFrame = null;
let currentSessionId = null;
let resizeFrame = null;
let transcriptState = window.TranscriptState.createEmptyTranscriptState();

function createBars() {
    const waveform = document.getElementById('waveform');
    for (let i = 0; i < NUM_BARS; i++) {
        const bar = document.createElement('div');
        bar.className = 'bar';
        bar.style.height = '3px';
        waveform.appendChild(bar);
        bars.push({ el: bar, target: 3, current: 3 });
    }
}

function updateBars(amplitude) {
    // Boost low amplitudes — mic RMS is typically 0.01-0.1
    // Use a curve that makes quiet speech visible
    const boosted = Math.pow(Math.min(amplitude * 8, 1), 0.6);

    // Track recent amplitudes for smoothing
    amplitudeHistory.push(boosted);
    if (amplitudeHistory.length > HISTORY_SIZE) amplitudeHistory.shift();
    const smoothed = amplitudeHistory.reduce((a, b) => a + b, 0) / amplitudeHistory.length;

    // Shift bars left
    for (let i = 0; i < bars.length - 1; i++) {
        bars[i].target = bars[i + 1].target;
    }

    // New bar with some randomized variation for a lively feel
    const variation = 0.7 + Math.random() * 0.6;
    const height = 3 + smoothed * variation * 37;
    bars[bars.length - 1].target = height;
}

function animateBars() {
    for (const bar of bars) {
        // Smooth interpolation toward target
        bar.current += (bar.target - bar.current) * 0.35;
        bar.el.style.height = Math.max(3, bar.current) + 'px';
    }
    animFrame = requestAnimationFrame(animateBars);
}

function stopAnimation() {
    if (animFrame) {
        cancelAnimationFrame(animFrame);
        animFrame = null;
    }
    amplitudeHistory = [];
    bars.forEach(bar => {
        bar.target = 3;
        bar.current = 3;
        bar.el.style.height = '3px';
    });
}

async function startAmplitudeStream() {
    amplitudeChannel = new Channel();
    amplitudeChannel.onmessage = (amplitude) => {
        updateBars(amplitude);
    };
    await invoke('start_amplitude_stream', { onAmplitude: amplitudeChannel });
}

async function handleStop() {
    await invoke('stop');
}

async function handleCancel() {
    await invoke('cancel');
}

function schedulePillResize() {
    if (resizeFrame !== null) {
        cancelAnimationFrame(resizeFrame);
    }

    resizeFrame = requestAnimationFrame(async () => {
        resizeFrame = null;
        const pillEl = document.getElementById('pill');
        const nextHeight = Math.ceil(pillEl.scrollHeight);
        try {
            await invoke('resize_pill', { height: nextHeight });
        } catch (error) {
            console.error('Failed to resize pill', error);
        }
    });
}

async function init() {
    createBars();
    document.getElementById('btn-stop').addEventListener('click', handleStop);
    document.getElementById('btn-cancel').addEventListener('click', handleCancel);
    const waveformEl = document.getElementById('waveform');
    const processingEl = document.getElementById('processing');
    const controlsEl = document.getElementById('controls');
    const transcriptEl = document.getElementById('transcript');
    const transcriptReferenceEl = document.getElementById('transcript-reference');
    const resolvingIndicatorEl = document.getElementById('resolving-indicator');
    const resolveTimingEl = document.getElementById('resolve-timing');

    function updateTranscriptVisibility() {
        const hasContent = Boolean(transcriptReferenceEl.textContent)
            || resolvingIndicatorEl.classList.contains('active');
        transcriptEl.classList.toggle('has-content', hasContent);
    }

    await listen('resolving-context', () => {
        // Show loading dots — always accept since this fires right after recording starts
        resolvingIndicatorEl.classList.add('active');
        updateTranscriptVisibility();
        schedulePillResize();
    });

    await listen('recording-state', (event) => {
        const state = event.payload;
        if (state === 'recording') {
            // Show waveform + controls, hide processing
            currentSessionId = null;
            transcriptState = window.TranscriptState.createEmptyTranscriptState();
            transcriptReferenceEl.textContent = '';
            transcriptEl.title = '';
            resolvingIndicatorEl.classList.remove('active');
            resolveTimingEl.textContent = '';
            resolveTimingEl.classList.remove('active');
            updateTranscriptVisibility();
            waveformEl.style.display = 'flex';
            controlsEl.style.display = 'flex';
            processingEl.classList.remove('active');
            startAmplitudeStream();
            animateBars();
            schedulePillResize();
        } else if (state === 'processing') {
            // Hide waveform + controls, show processing spinner
            stopAnimation();
            waveformEl.style.display = 'none';
            controlsEl.style.display = 'none';
            processingEl.classList.add('active');
            schedulePillResize();
        } else {
            // Idle — reset everything
            stopAnimation();
            waveformEl.style.display = 'flex';
            controlsEl.style.display = 'flex';
            processingEl.classList.remove('active');
            schedulePillResize();
        }
    });

    await listen('transcript-update', (event) => {
        const payload = event.payload || {};
        const sessionId = payload.session_id;

        if (currentSessionId === null) {
            currentSessionId = sessionId;
        }

        if (sessionId !== currentSessionId) {
            return;
        }

        transcriptState = window.TranscriptState.mergeTranscriptUpdate(
            transcriptState,
            payload,
        );

        transcriptReferenceEl.textContent = transcriptState.referenceText;
        transcriptEl.title = transcriptState.referenceText;
        if (transcriptState.referenceText) {
            resolvingIndicatorEl.classList.remove('active');
            if (transcriptState.resolveMs !== null) {
                resolveTimingEl.textContent = `resolved in ${transcriptState.resolveMs}ms`;
                resolveTimingEl.classList.add('active');
            }
        }
        updateTranscriptVisibility();
        schedulePillResize();
    });

    schedulePillResize();
}

init();
