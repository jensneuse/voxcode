(function (global, factory) {
    const api = factory();
    if (typeof module !== 'undefined' && module.exports) {
        module.exports = api;
    }
    global.TranscriptState = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
    function createEmptyTranscriptState() {
        return { referenceText: '', resolveMs: null };
    }

    function mergeTranscriptUpdate(currentState, payload) {
        const current = currentState || createEmptyTranscriptState();
        const nextReference = payload.reference_text ?? '';
        return {
            referenceText: nextReference || current.referenceText,
            resolveMs: payload.resolve_ms ?? current.resolveMs,
        };
    }

    return {
        createEmptyTranscriptState,
        mergeTranscriptUpdate,
    };
});
