"""
Utils module.
"""

from .constants import reserved_keys


def drop_suffixes(d, skipdups=True):
    """
    Merge suffixes for same var, or drop off unnecessary suffixes

    This step returns a copy of a suffix flattened dictionary.
    """
    # dictionary `d_flat' is going to be the mutated copy of `d`
    d_flat = d.copy()
    for key in d:
        if key in reserved_keys:
            continue

        if not isinstance(key, tuple):
            continue

        if skipdups:
            # Drop vars with suffixes matches general var val
            # Example: if a_x == 1, and a == 1. Drop: a_x, leave a
            gen_var_name = key[0]
            if gen_var_name in d and d[gen_var_name] == d[key]:
                # Drop gen_var_name, use general key with same value
                d_flat.pop(key)
                continue

            can_drop_all_suffixes_for_this_key = True
            for k in d:
                gen_name = k[0] if isinstance(k, tuple) else k
                if gen_var_name == gen_name:
                    if d[key] != d[k]:
                        can_drop_all_suffixes_for_this_key = False
                        break

        if skipdups and can_drop_all_suffixes_for_this_key:
            new_key = key[0]
        else:
            # merge suffixes, preserve reverse order of suffixes
            new_key = key[:1] + key[1:][::-1]
            new_key = ''.join((map(str, new_key)))
        d_flat[new_key] = d_flat.pop(key)

    return d_flat
